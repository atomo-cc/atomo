//! HTTP-level tests for the /media endpoints (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test media_http -- --ignored --test-threads=1
//! Covers the security surface: auth enforcement, multipart, content-type allowlist, size limit,
//! serve headers, and the delete lifecycle — driven through the real axum router.

use atomo_server::auth::HttpAuthService;
use atomo_server::media::{media_router, MediaState};
use atomo_server::storage::LocalStorage;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

/// Minimal valid PNG signature (passes magic-byte sniffing).
const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

async fn connect() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    atomo_server::ensure_platform_tables(&pool).await.unwrap();
    pool
}

async fn seed_user_token(pool: &sqlx::PgPool, auth: &HttpAuthService, tenant: Option<&str>) -> String {
    let id = atomo_core::types::EntityId::new().to_string();
    let email = format!("u-{}@test.dev", id);
    sqlx::query(
        "INSERT INTO users (id,email,password_hash,first_name,last_name,role,is_active,tenant_id)
         VALUES ($1,$2,'x','U','R','admin',true,$3)",
    )
    .bind(&id)
    .bind(&email)
    .bind(tenant)
    .execute(pool)
    .await
    .unwrap();
    auth.issue_tokens(&id, &email, "admin").await.unwrap().0
}

fn multipart(filename: &str, content_type: &str, data: &[u8]) -> (String, Body) {
    let b = "BOUNDARYtest123";
    let mut buf = Vec::new();
    buf.extend_from_slice(
        format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    buf.extend_from_slice(data);
    buf.extend_from_slice(format!("\r\n--{b}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={b}"), Body::from(buf))
}

fn upload_req(token: Option<&str>, content_type: &str, data: &[u8]) -> Request<Body> {
    let (ct, body) = multipart("a.png", content_type, data);
    let mut b = Request::builder()
        .method("POST")
        .uri("/media")
        .header("Content-Type", ct);
    if let Some(t) = token {
        b = b.header("Authorization", format!("Bearer {t}"));
    }
    b.body(body).unwrap()
}

#[tokio::test]
#[ignore]
async fn media_http_full_lifecycle_and_security() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let token = seed_user_token(&pool, &auth, None).await;
    let dir = std::env::temp_dir().join(format!("atomo-media-http-{}", uuid::Uuid::new_v4()));
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(MediaState::new(pool.clone(), Arc::new(LocalStorage::new(&dir)), tx));
    state.init().await.unwrap();
    let app = media_router(state, auth);

    // 1. upload without auth -> 401
    let r = app
        .clone()
        .oneshot(upload_req(None, "image/png", b"PNGDATA"))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "upload requires auth");

    // 2. disallowed content-type -> 415
    let r = app
        .clone()
        .oneshot(upload_req(Some(&token), "text/html", b"<script>"))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE, "html blocked");

    // 2b. declared image/png but bytes aren't a PNG -> 415 (magic-byte sniff)
    let r = app
        .clone()
        .oneshot(upload_req(Some(&token), "image/png", b"not-a-real-png"))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE, "content/type mismatch blocked");

    // 3. valid upload -> 200 {id,url}
    let r = app
        .clone()
        .oneshot(upload_req(Some(&token), "image/png", PNG))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(r.into_body(), 1_000_000).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = json["id"].as_str().unwrap().to_string();
    assert_eq!(json["contentType"], "image/png");

    // 4. serve (public) -> 200 with content-type + nosniff + correct bytes
    let r = app
        .clone()
        .oneshot(Request::builder().uri(format!("/media/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers().get("content-type").unwrap(), "image/png");
    assert_eq!(r.headers().get("x-content-type-options").unwrap(), "nosniff");
    let served = axum::body::to_bytes(r.into_body(), 1_000_000).await.unwrap();
    assert_eq!(served.as_ref(), PNG);

    // 5. delete without auth -> 401
    let r = app
        .clone()
        .oneshot(Request::builder().method("DELETE").uri(format!("/media/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "delete requires auth");

    // 6. delete with auth -> 204, then serve -> 404
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/media/{id}"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    let r = app
        .clone()
        .oneshot(Request::builder().uri(format!("/media/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND, "deleted media not served");

    // 7. unknown id -> 404
    let r = app
        .clone()
        .oneshot(Request::builder().uri("/media/does-not-exist").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    // 8. GC endpoint: requires auth + admin; returns a purge count
    let r = app
        .clone()
        .oneshot(Request::builder().method("POST").uri("/media/gc").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "gc requires auth");
    let r = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/media/gc?older_than_secs=0")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK, "admin gc ok");

    tokio::fs::remove_dir_all(&dir).await.ok();
}

#[tokio::test]
#[ignore]
async fn media_http_rejects_oversized_body() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let token = seed_user_token(&pool, &auth, None).await;
    let dir = std::env::temp_dir().join(format!("atomo-media-sz-{}", uuid::Uuid::new_v4()));
    std::env::set_var("STORAGE_MAX_FILE_SIZE", "8"); // tiny cap -> any multipart body exceeds
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(MediaState::new(pool.clone(), Arc::new(LocalStorage::new(&dir)), tx));
    state.init().await.unwrap();
    let app = media_router(state, auth);
    std::env::remove_var("STORAGE_MAX_FILE_SIZE");

    let r = app
        .oneshot(upload_req(Some(&token), "image/png", b"way-too-large-payload"))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::PAYLOAD_TOO_LARGE);
    tokio::fs::remove_dir_all(&dir).await.ok();
}

#[tokio::test]
#[ignore]
async fn media_http_private_reads_are_tenant_scoped() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let t1 = seed_user_token(&pool, &auth, Some("t1")).await;
    let t2 = seed_user_token(&pool, &auth, Some("t2")).await;
    let dir = std::env::temp_dir().join(format!("atomo-media-priv-{}", uuid::Uuid::new_v4()));
    std::env::set_var("STORAGE_PRIVATE_READS", "true");
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(MediaState::new(pool.clone(), Arc::new(LocalStorage::new(&dir)), tx));
    state.init().await.unwrap();
    let app = media_router(state, auth);
    std::env::remove_var("STORAGE_PRIVATE_READS");

    // upload as tenant t1
    let r = app
        .clone()
        .oneshot(upload_req(Some(&t1), "image/png", PNG))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(r.into_body(), 1_000_000).await.unwrap();
    let id = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let get = |token: Option<&str>| {
        let mut b = Request::builder().uri(format!("/media/{id}"));
        if let Some(t) = token {
            b = b.header("Authorization", format!("Bearer {t}"));
        }
        b.body(Body::empty()).unwrap()
    };

    // no token -> 401; wrong tenant -> 403; owning tenant -> 200
    assert_eq!(app.clone().oneshot(get(None)).await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(app.clone().oneshot(get(Some(&t2))).await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(app.oneshot(get(Some(&t1))).await.unwrap().status(), StatusCode::OK);

    tokio::fs::remove_dir_all(&dir).await.ok();
}
