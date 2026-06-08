//! Integration test: full action lifecycle.
//!
//!   create Post → action enqueued → worker leases → worker CRUD update with origin
//!   → update event written → origin prevents loop → different action still fires
//!
//! Requires a running Postgres instance via DATABASE_URL.
//! Run with: cargo test -p atomo_server --test action_lifecycle -- --ignored

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

const SCHEMA_TS: &str = r#"
export interface Post {
  id: string;
  title: string;
  status: string;
}

export const schema = {
  models: {
    Post: {
      tableName: "lifecycle_test_posts",
      events: {
        created: [{ action: "processPost" }],
        updated: [
          { action: "onStatusChange", condition: { changedAny: ["status"] } },
          { action: "processPost" },
        ],
      },
    },
  },
  actions: {
    processPost: {
      input: { pick: { model: "Post", fields: ["id", "title"] } },
    },
    onStatusChange: {
      input: { pick: { model: "Post", fields: ["id", "status"] } },
    },
  },
};
export default schema;
"#;

async fn setup() -> (
    atomo::Atomo,
    Arc<atomo_server::jobs::JobStore>,
    Arc<atomo_server::jobs::WorkerTokenStore>,
    axum::Router,
) {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let atomo = atomo::Atomo::builder()
        .schema_content(SCHEMA_TS)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();

    let job_store = Arc::new(atomo_server::jobs::JobStore::new(
        atomo.db_pool().clone(),
        atomo.event_sender(),
    ));
    job_store.init().await.unwrap();

    let worker_tokens = Arc::new(atomo_server::jobs::WorkerTokenStore::new(
        atomo.db_pool().clone(),
    ));
    worker_tokens.init().await.unwrap();

    // Spawn the action dispatcher (bridges model events → job queue).
    atomo_server::action_dispatcher::spawn_action_dispatcher(
        atomo.schema().clone(),
        job_store.clone(),
        atomo.event_receiver(),
    );

    // Build the CRUD router (what workers call back into).
    let crud_router = atomo_server::crud_routes::crud_router(
        Arc::new(atomo.client().clone()),
        atomo.schema().clone(),
        worker_tokens.clone(),
    );

    // Build the jobs router (lease/complete/fail).
    let jobs_router = atomo_server::job_routes::jobs_router(
        job_store.clone(),
        worker_tokens.clone(),
        atomo_server::auth::HttpAuthService::new("test-secret", atomo.db_pool().clone()),
        None,
    );

    let app = crud_router.merge(jobs_router);

    (atomo, job_store, worker_tokens, app)
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

// ── Full lifecycle test ─────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn lifecycle_create_enqueue_worker_crud_update_origin_loop_prevention() {
    let (atomo, _job_store, worker_tokens, app) = setup().await;

    // Clean up from previous runs.
    sqlx::query("DELETE FROM lifecycle_test_posts")
        .execute(atomo.db_pool())
        .await
        .ok();
    sqlx::query("DELETE FROM jobs WHERE queue = 'actions'")
        .execute(atomo.db_pool())
        .await
        .ok();

    // ── Step 1: Create a Post (triggers "processPost" action) ───────────

    let mut data = std::collections::HashMap::new();
    data.insert("title".to_string(), json!("Hello World"));
    data.insert("status".to_string(), json!("draft"));
    let post = atomo
        .client()
        .create("Post", &data, &[], Some("user-1"))
        .await
        .unwrap();
    let post_id = post["id"].as_str().unwrap().to_string();

    // Give the dispatcher a moment to process the event.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // ── Step 2: Verify processPost was enqueued ─────────────────────────

    let pending: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE queue = 'actions' AND kind = 'processPost' AND status = 'pending'")
            .fetch_one(atomo.db_pool())
            .await
            .unwrap();
    assert!(
        pending.0 >= 1,
        "processPost should be enqueued after Post.created, got {}",
        pending.0
    );

    // ── Step 3: Mint a worker token and lease the job ───────────────────

    let (_, token) = worker_tokens
        .mint("test-worker", &["actions".into()], &["crud:*".into()])
        .await
        .unwrap();

    let lease_req = Request::builder()
        .uri("/jobs/lease")
        .method("POST")
        .header("content-type", "application/json")
        .header("x-worker-token", &token)
        .body(Body::from(
            json!({"queues": ["actions"], "capacity": 5, "visibilitySecs": 30}).to_string(),
        ))
        .unwrap();
    let (status, lease_json) = send(&app, lease_req).await;
    assert_eq!(status, StatusCode::OK, "lease failed: {:?}", lease_json);
    let jobs = lease_json["jobs"].as_array().expect("jobs array");
    assert!(
        !jobs.is_empty(),
        "should have leased at least one processPost job"
    );

    let leased_job = &jobs[0];
    let job_id = leased_job["id"].as_str().unwrap();
    let lease_id = leased_job["leaseId"].as_str().unwrap();
    assert_eq!(leased_job["kind"], "processPost");

    // ── Step 4: Worker calls CRUD update with origin ────────────────────

    // Clear old jobs so we can count new enqueues precisely.
    sqlx::query("DELETE FROM jobs WHERE queue = 'actions' AND status = 'pending'")
        .execute(atomo.db_pool())
        .await
        .ok();

    // Record event_log count before the update.
    let before_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM event_log WHERE model_name = 'Post' AND event_type = 'Updated'",
    )
    .fetch_one(atomo.db_pool())
    .await
    .unwrap();

    let update_req = Request::builder()
        .uri(format!("/api/worker/crud/Post/{}", post_id))
        .method("PATCH")
        .header("content-type", "application/json")
        .header("x-worker-token", &token)
        .body(Body::from(
            json!({
                "data": { "title": "Processed Title", "status": "published" },
                "actor": { "userId": "user-1", "role": null },
                "origin": "processPost"
            })
            .to_string(),
        ))
        .unwrap();
    let (status, update_json) = send(&app, update_req).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "worker CRUD update failed: {:?}",
        update_json
    );
    assert_eq!(
        update_json["title"], "Processed Title",
        "update should return the modified record"
    );

    // ── Step 5: Complete the job ────────────────────────────────────────

    let complete_req = Request::builder()
        .uri(format!("/jobs/{}/complete", job_id))
        .method("POST")
        .header("content-type", "application/json")
        .header("x-worker-token", &token)
        .body(Body::from(
            json!({"leaseId": lease_id, "result": {"processed": true}}).to_string(),
        ))
        .unwrap();
    let (status, _) = send(&app, complete_req).await;
    assert!(
        status == StatusCode::OK || status == StatusCode::NO_CONTENT,
        "complete failed: {}",
        status
    );

    // Give the dispatcher time to process the update event.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // ── Step 6: Verify event sourcing ───────────────────────────────────

    let after_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM event_log WHERE model_name = 'Post' AND event_type = 'Updated'",
    )
    .fetch_one(atomo.db_pool())
    .await
    .unwrap();
    assert!(
        after_count.0 > before_count.0,
        "an Updated event should have been written to event_log"
    );

    // ── Step 7: Verify origin-based loop prevention ─────────────────────

    // processPost should NOT have been re-enqueued (origin == "processPost").
    let requeued: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs WHERE queue = 'actions' AND kind = 'processPost' AND status = 'pending'",
    )
    .fetch_one(atomo.db_pool())
    .await
    .unwrap();
    assert_eq!(
        requeued.0, 0,
        "processPost must NOT be re-enqueued when origin = 'processPost' (loop prevention)"
    );

    // onStatusChange SHOULD have been enqueued (status changed from draft → published,
    // and origin = 'processPost' != 'onStatusChange').
    let status_change: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs WHERE queue = 'actions' AND kind = 'onStatusChange' AND status = 'pending'",
    )
    .fetch_one(atomo.db_pool())
    .await
    .unwrap();
    assert!(
        status_change.0 >= 1,
        "onStatusChange should be enqueued (status changed and origin != 'onStatusChange')"
    );

    // ── Step 8: Verify the post was actually updated in the DB ──────────

    let found = atomo
        .client()
        .find_unique(
            "Post",
            &[atomo::query::WhereClause {
                field: "id".into(),
                operator: atomo::query::WhereOperator::Equals,
                value: json!(post_id),
            }],
            &[],
        )
        .await
        .unwrap()
        .expect("post should still exist");
    assert_eq!(found["title"], "Processed Title");
    assert_eq!(found["status"], "published");

    // ── Cleanup ─────────────────────────────────────────────────────────

    sqlx::query("DROP TABLE IF EXISTS lifecycle_test_posts CASCADE")
        .execute(atomo.db_pool())
        .await
        .ok();
}
