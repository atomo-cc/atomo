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

async fn seed_token(pool: &sqlx::PgPool, auth: &HttpAuthService, role: &str) -> String {
    let id = atomo_core::types::EntityId::new().to_string();
    let email = format!("{role}-{id}@test.dev");
    sqlx::query(
        "INSERT INTO users (id,email,password_hash,first_name,last_name,role,is_active)
         VALUES ($1,$2,'x','U','R',$3,true)",
    )
    .bind(&id)
    .bind(&email)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    auth.issue_tokens(&id, &email, role).await.unwrap().0
}

async fn seed_admin_token(pool: &sqlx::PgPool, auth: &HttpAuthService) -> String {
    seed_token(pool, auth, "admin").await
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
    let app = jobs_router(jobs.clone(), workers.clone(), auth, None);

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

    // App-side enqueue requires an authenticated user.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs",
            &[],
            json!({"queue": allowed, "kind": "video.generate"}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "enqueue needs auth");

    // Enqueue over HTTP as the admin user.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs",
            &[("authorization", &format!("Bearer {admin}"))],
            json!({"queue": allowed, "kind": "video.generate", "payload": {"prompt": "hi"}}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let job_id = json_body(r).await["id"].as_str().unwrap().to_string();

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

    // App polls the job over HTTP and sees the terminal state + result.
    let get_job = |token: Option<&str>| {
        let mut b = Request::builder()
            .method("GET")
            .uri(format!("/jobs/{job_id}"));
        if let Some(t) = token {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        b.body(Body::empty()).unwrap()
    };
    let r = app.clone().oneshot(get_job(Some(&admin))).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let view = json_body(r).await;
    assert_eq!(view["status"], "succeeded");
    assert_eq!(view["result"]["assetId"], "abc");
    // Status poll needs auth.
    assert_eq!(
        app.clone().oneshot(get_job(None)).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
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

#[tokio::test]
#[ignore]
async fn jobs_http_enqueue_validation_idempotency_and_fail_outcomes() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let admin = seed_admin_token(&pool, &auth).await;
    let viewer = seed_token(&pool, &auth, "viewer").await;

    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let jobs = Arc::new(JobStore::new(pool.clone(), tx));
    jobs.init().await.unwrap();
    let workers = Arc::new(WorkerTokenStore::new(pool.clone()));
    workers.init().await.unwrap();
    let app = jobs_router(jobs.clone(), workers.clone(), auth, None);
    let queue = format!("q-{}", uuid::Uuid::new_v4());

    // A non-admin user cannot mint worker tokens.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs/workers",
            &[("authorization", &format!("Bearer {viewer}"))],
            json!({"name": "x", "queues": [queue]}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN, "mint is admin-only");

    // Enqueue validation: an empty kind → 400 (the handler rejects blank queue/kind).
    let r = app
        .clone()
        .oneshot(post(
            "/jobs",
            &[("authorization", &format!("Bearer {admin}"))],
            json!({"queue": queue, "kind": ""}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // Idempotent enqueue: same key → same id.
    let enq = |key: &str| {
        post(
            "/jobs",
            &[("authorization", &format!("Bearer {admin}"))],
            json!({"queue": queue, "kind": "k", "idempotencyKey": key}),
        )
    };
    let r1 = app.clone().oneshot(enq("dupe")).await.unwrap();
    assert_eq!(r1.status(), StatusCode::CREATED);
    let id1 = json_body(r1).await["id"].as_str().unwrap().to_string();
    let r2 = app.clone().oneshot(enq("dupe")).await.unwrap();
    let id2 = json_body(r2).await["id"].as_str().unwrap().to_string();
    assert_eq!(id1, id2, "same idempotency key returns the same job");

    // Mint a worker token and lease the job to exercise the fail outcomes.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs/workers",
            &[("authorization", &format!("Bearer {admin}"))],
            json!({"name": "w", "queues": [queue]}),
        ))
        .await
        .unwrap();
    let token = json_body(r).await["token"].as_str().unwrap().to_string();

    let r = app
        .clone()
        .oneshot(post(
            "/jobs/lease",
            &[("x-worker-token", &token)],
            json!({"queues": [queue]}),
        ))
        .await
        .unwrap();
    let arr = json_body(r).await;
    let job = &arr["jobs"][0];
    let job_id = job["id"].as_str().unwrap().to_string();
    let lease_id = job["leaseId"].as_str().unwrap().to_string();

    // Heartbeat with a bogus lease → 409.
    let r = app
        .clone()
        .oneshot(post(
            &format!("/jobs/{job_id}/heartbeat"),
            &[("x-worker-token", &token)],
            json!({"leaseId": "nope"}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);

    // Fail (retryable) → 200 { outcome: "retry" }; the job returns to the queue.
    let r = app
        .clone()
        .oneshot(post(
            &format!("/jobs/{job_id}/fail"),
            &[("x-worker-token", &token)],
            json!({"leaseId": lease_id, "error": "boom", "retryable": true}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(json_body(r).await["outcome"], "retry");
    assert_eq!(
        jobs.status(&job_id).await.unwrap().as_deref(),
        Some("queued")
    );

    // Failing again with the now-stale lease → 409 (no-op).
    let r = app
        .oneshot(post(
            &format!("/jobs/{job_id}/fail"),
            &[("x-worker-token", &token)],
            json!({"leaseId": lease_id, "error": "again", "retryable": true}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore]
async fn jobs_http_worker_token_list_and_revoke() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let admin = seed_admin_token(&pool, &auth).await;
    let viewer = seed_token(&pool, &auth, "viewer").await;

    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let jobs = Arc::new(JobStore::new(pool.clone(), tx));
    jobs.init().await.unwrap();
    let workers = Arc::new(WorkerTokenStore::new(pool.clone()));
    workers.init().await.unwrap();
    let app = jobs_router(jobs, workers, auth, None);
    let queue = format!("q-{}", uuid::Uuid::new_v4());

    let admin_hdr = [("authorization", format!("Bearer {admin}"))];
    let admin_h: Vec<(&str, &str)> = admin_hdr.iter().map(|(k, v)| (*k, v.as_str())).collect();

    // Mint a token.
    let r = app
        .clone()
        .oneshot(post(
            "/jobs/workers",
            &admin_h,
            json!({"name": "w", "queues": [queue]}),
        ))
        .await
        .unwrap();
    let minted = json_body(r).await;
    let token = minted["token"].as_str().unwrap().to_string();
    let token_id = minted["id"].as_str().unwrap().to_string();

    // List requires admin; a non-admin → 403.
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/jobs/workers")
                .header("authorization", format!("Bearer {viewer}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    // Admin list shows the token (metadata only, not revoked, no secret).
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/jobs/workers")
                .header("authorization", format!("Bearer {admin}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let listed = json_body(r).await;
    let mine = listed["workers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == token_id)
        .expect("token appears in the list");
    assert_eq!(mine["isRevoked"], false);
    assert!(mine.get("token").is_none() && mine.get("tokenSha256").is_none());

    // The token works before revocation (leasing its queue is allowed → 200).
    let r = app
        .clone()
        .oneshot(post(
            "/jobs/lease",
            &[("x-worker-token", &token)],
            json!({"queues": [queue]}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Revoke (admin). A non-admin → 403; revoking twice → 404.
    let revoke = |hdr: Option<&str>| {
        let mut b = Request::builder()
            .method("DELETE")
            .uri(format!("/jobs/workers/{token_id}"));
        if let Some(h) = hdr {
            b = b.header("authorization", format!("Bearer {h}"));
        }
        b.body(Body::empty()).unwrap()
    };
    assert_eq!(
        app.clone()
            .oneshot(revoke(Some(&viewer)))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone()
            .oneshot(revoke(Some(&admin)))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.clone()
            .oneshot(revoke(Some(&admin)))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND,
        "revoking an already-revoked token is 404"
    );

    // The revoked token can no longer lease → 401.
    let r = app
        .oneshot(post(
            "/jobs/lease",
            &[("x-worker-token", &token)],
            json!({"queues": [queue]}),
        ))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::UNAUTHORIZED,
        "revoked token rejected"
    );
}

#[tokio::test]
#[ignore]
async fn jobs_http_progress_publishes_to_realtime() {
    use atomo_realtime::{ClientMsg, Hub, Principal, ServerMsg};

    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let jobs = Arc::new(JobStore::new(pool.clone(), tx));
    jobs.init().await.unwrap();
    let workers = Arc::new(WorkerTokenStore::new(pool.clone()));
    workers.init().await.unwrap();
    let queue = format!("media-{}", uuid::Uuid::new_v4());

    // Enqueue + lease a job directly to get a valid lease.
    let job_id = jobs
        .enqueue(&queue, "video.generate", json!({}), None, 5, 0, None)
        .await
        .unwrap();
    let leased = jobs
        .lease(std::slice::from_ref(&queue), 1, 30)
        .await
        .unwrap();
    let lease_id = leased[0].lease_id.clone();
    let (_tok_id, token) = workers
        .mint("w", std::slice::from_ref(&queue), &[])
        .await
        .unwrap();

    // Hub: a watcher subscribes to the job's channel; the server gets a system publisher.
    let hub = Hub::new();
    let mut watcher = hub.connect(Principal::new("watcher", None)).await;
    let channel = format!("job:{job_id}");
    watcher
        .handle
        .dispatch(ClientMsg::Subscribe {
            channel: channel.clone(),
        })
        .await;
    let publisher = hub
        .connect(Principal::new("system:jobs", None))
        .await
        .handle;
    // Let the subscribe command register before publishing.
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;

    let app = jobs_router(jobs, workers, auth, Some(publisher));

    // Report progress with the valid lease → 204.
    let r = app
        .clone()
        .oneshot(post(
            &format!("/jobs/{job_id}/progress"),
            &[("x-worker-token", &token)],
            json!({"leaseId": lease_id, "percent": 0.5, "message": "halfway"}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // The watcher receives the progress on the job channel (skipping any presence frames).
    let mut got = None;
    for _ in 0..5 {
        match tokio::time::timeout(std::time::Duration::from_secs(2), watcher.outbound.recv()).await
        {
            Ok(Some(ServerMsg::Message {
                channel: ch,
                payload,
                ..
            })) if ch == channel => {
                got = Some(serde_json::from_str::<Value>(payload.get()).unwrap());
                break;
            }
            Ok(Some(_)) => continue, // presence/joined — ignore
            _ => break,
        }
    }
    let msg = got.expect("watcher received a progress message");
    assert_eq!(msg["jobId"], job_id);
    assert_eq!(msg["percent"], 0.5);
    assert_eq!(msg["message"], "halfway");

    // A bogus lease → 409 (and nothing published).
    let r = app
        .oneshot(post(
            &format!("/jobs/{job_id}/progress"),
            &[("x-worker-token", &token)],
            json!({"leaseId": "nope", "message": "x"}),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore]
async fn jobs_http_observability_stats_and_recent() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let admin = seed_admin_token(&pool, &auth).await;
    let viewer = seed_token(&pool, &auth, "viewer").await;

    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let jobs = Arc::new(JobStore::new(pool.clone(), tx));
    jobs.init().await.unwrap();
    let workers = Arc::new(WorkerTokenStore::new(pool.clone()));
    workers.init().await.unwrap();
    let app = jobs_router(jobs.clone(), workers.clone(), auth, None);

    // Unique queue per run so counts are deterministic on a shared database.
    let queue = format!("obs-{}", uuid::Uuid::new_v4());
    for i in 0..3 {
        jobs.enqueue(&queue, "obsJob", json!({"i": i}), None, 5, 0, None)
            .await
            .unwrap();
    }

    // Admin-only: anon and non-admin are refused.
    let get = |uri: &str, token: Option<&str>| {
        let mut b = Request::builder().method("GET").uri(uri);
        if let Some(t) = token {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        b.body(Body::empty()).unwrap()
    };
    let r = app.clone().oneshot(get("/jobs/stats", None)).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "stats needs auth");
    let r = app
        .clone()
        .oneshot(get("/jobs/stats", Some(&viewer)))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN, "stats is admin-only");

    // Stats: our three queued jobs show under byQueue for this queue.
    let r = app
        .clone()
        .oneshot(get("/jobs/stats", Some(&admin)))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let stats = json_body(r).await;
    let queued_here = stats["byQueue"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["queue"] == queue.as_str() && q["status"] == "queued")
        .expect("our queue appears in byQueue");
    assert_eq!(queued_here["count"], 3);
    assert!(stats["byStatus"]["queued"].as_i64().unwrap() >= 3);
    assert!(stats["oldestQueuedSeconds"].as_f64().is_some());

    // Recent: filterable by queue + status, newest first, no payload bodies.
    let r = app
        .clone()
        .oneshot(get(
            &format!("/jobs/recent?queue={queue}&status=queued&limit=2"),
            Some(&admin),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = json_body(r).await;
    let items = body["jobs"].as_array().unwrap();
    assert_eq!(items.len(), 2, "limit honored");
    assert!(items.iter().all(|j| j["queue"] == queue.as_str()));
    assert!(items[0].get("payload").is_none(), "no payload bodies");
    assert!(items[0]["createdAt"].as_str().unwrap().contains('T'));
}
