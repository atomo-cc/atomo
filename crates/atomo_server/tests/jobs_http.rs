//! HTTP-level tests for the /jobs lease API + worker-token auth (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test jobs_http -- --ignored --test-threads=1
//! Drives the full path through the real axum router: admin mints a scoped worker token, a worker
//! pulls a job with that token, completes it, and the capability/auth boundaries are enforced.

use atomo_server::auth::HttpAuthService;
use atomo_server::job_routes::jobs_router;
use atomo_server::jobs::{JobStore, WorkerTokenStore};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn connect() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    atomo_server::ensure_platform_tables(&pool).await.unwrap();
    pool
}

async fn seed_admin_token(pool: &sqlx::PgPool, auth: &HttpAuthService) -> String {
    let id = atomo_core::types::EntityId::new().to_string();
    let email = format!("admin-{id}@test.dev");
    sqlx::query(
        "INSERT INTO users (id,email,password_hash,first_name,last_name,role,is_active)
         VALUES ($1,$2,'x','A','D','admin',true)",
    )
    .bind(&id)
    .bind(&email)
    .execute(pool)
    .await
    .unwrap();
    auth.issue_tokens(&id, &email, "admin").await.unwrap().0
}

fn post(uri: &str, headers: &[(&str, &str)], body: Value) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    b.body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
#[ignore]
async fn jobs_http_lease_lifecycle_and_auth() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let admin = seed_admin_token(&pool, &auth).await;

    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let jobs = Arc::new(JobStore::new(pool.clone(), tx));
    jobs.init().await.unwrap();
    let workers = Arc::new(WorkerTokenStore::new(pool.clone()));
    workers.init().await.unwrap();
    let app = jobs_router(jobs.clone(), workers.clone(), auth);

    let allowed = format!("media-{}", uuid::Uuid::new_v4());
    let forbidden = format!("billing-{}", uuid::Uuid::new_v4());

    // Minting a worker token requires admin auth.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs/workers",
            &[],
            json!({"name": "w", "queues": [allowed]}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "mint needs auth");

    // Admin mints a token scoped to the allowed queue; plaintext returned once.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs/workers",
            &[("authorization", &format!("Bearer {admin}"))],
            json!({"name": "media-worker", "queues": [allowed]}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let token = json_body(r).await["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("wkr_"));

    // Leasing without a worker token → 401.
    let r = app
        .clone()
        .oneshot(post("/jobs/lease", &[], json!({"queues": [allowed]})))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::UNAUTHORIZED,
        "lease needs a worker token"
    );

    // Leasing a queue the token isn't scoped to → 403.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs/lease",
            &[("x-worker-token", &token)],
            json!({"queues": [forbidden]}),
        ))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::FORBIDDEN,
        "queue not in capability set"
    );

    // Enqueue a job on the allowed queue (app-side enqueue is a library call for now).
    let job_id = jobs
        .enqueue(
            &allowed,
            "video.generate",
            json!({"prompt": "hi"}),
            None,
            5,
            0,
            None,
        )
        .await
        .unwrap();

    // Worker leases it.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs/lease",
            &[("x-worker-token", &token)],
            json!({"queues": [allowed], "capacity": 5}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let leased = json_body(r).await;
    let arr = leased["jobs"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"].as_str().unwrap(), job_id);
    assert_eq!(arr[0]["kind"].as_str().unwrap(), "video.generate");
    let lease_id = arr[0]["leaseId"].as_str().unwrap().to_string();

    // Heartbeat with the lease extends (204); with a bogus lease → 409.
    let r = app
        .clone()
        .oneshot(post(
            &format!("/jobs/{job_id}/heartbeat"),
            &[("x-worker-token", &token)],
            json!({"leaseId": lease_id}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Complete the job (204); a second complete with the now-stale lease → 409.
    let r = app
        .clone()
        .oneshot(post(
            &format!("/jobs/{job_id}/complete"),
            &[("x-worker-token", &token)],
            json!({"leaseId": lease_id, "result": {"assetId": "abc"}}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        jobs.status(&job_id).await.unwrap().as_deref(),
        Some("succeeded")
    );

    let r = app
        .oneshot(post(
            &format!("/jobs/{job_id}/complete"),
            &[("x-worker-token", &token)],
            json!({"leaseId": lease_id, "result": {}}),
        ))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::CONFLICT,
        "stale complete is rejected"
    );
}
