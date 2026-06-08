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

async fn seed_user_token(
    pool: &sqlx::PgPool,
    auth: &HttpAuthService,
    tenant: Option<&str>,
) -> String {
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

async fn json_body(resp: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
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
    (
        format!("multipart/form-data; boundary={b}"),
        Body::from(buf),
    )
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
    let state = Arc::new(MediaState::new(
        pool.clone(),
        Arc::new(LocalStorage::new(&dir)),
        tx,
    ));
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
    assert_eq!(
        r.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "html blocked"
    );

    // 2b. declared image/png but bytes aren't a PNG -> 415 (magic-byte sniff)
    let r = app
        .clone()
        .oneshot(upload_req(Some(&token), "image/png", b"not-a-real-png"))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "content/type mismatch blocked"
    );

    // 3. valid upload -> 200 {id,url}. Unique PNG (valid signature + random tail) so
    // content-addressed dedup never reuses a prior run's media (bytes in a deleted temp dir).
    let mut png = PNG.to_vec();
    png.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    let r = app
        .clone()
        .oneshot(upload_req(Some(&token), "image/png", &png))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(r.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = json["id"].as_str().unwrap().to_string();
    assert_eq!(json["contentType"], "image/png");

    // 4. serve (public) -> 200 with content-type + nosniff + correct bytes
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/media/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers().get("content-type").unwrap(), "image/png");
    assert_eq!(
        r.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    let served = axum::body::to_bytes(r.into_body(), 1_000_000)
        .await
        .unwrap();
    assert_eq!(served.as_ref(), png.as_slice());

    // 5. delete without auth -> 401
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/media/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
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
        .oneshot(
            Request::builder()
                .uri(format!("/media/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::NOT_FOUND,
        "deleted media not served"
    );

    // 7. unknown id -> 404
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/media/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    // 8. GC endpoint: requires auth + admin; returns a purge count
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/media/gc")
                .body(Body::empty())
                .unwrap(),
        )
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
async fn media_http_supports_range_requests() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let token = seed_user_token(&pool, &auth, None).await;
    let dir = std::env::temp_dir().join(format!("atomo-media-range-{}", uuid::Uuid::new_v4()));
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(MediaState::new(
        pool.clone(),
        Arc::new(LocalStorage::new(&dir)),
        tx,
    ));
    state.init().await.unwrap();
    let app = media_router(state, auth);

    // A 26-byte payload: fixed ends ("ABCDEF…XYZ") for the range asserts, unique middle so
    // content-addressed dedup never reuses another run's media (whose bytes live in a temp dir).
    let mut data = b"ABCDEF".to_vec();
    data.extend_from_slice(&uuid::Uuid::new_v4().simple().to_string().as_bytes()[..17]);
    data.extend_from_slice(b"XYZ");
    assert_eq!(data.len(), 26);
    let r = app
        .clone()
        .oneshot(upload_req(Some(&token), "application/octet-stream", &data))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(r.into_body(), 1_000_000)
        .await
        .unwrap();
    let id = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let get = |range: Option<&str>, inm: Option<&str>| {
        let mut b = Request::builder().uri(format!("/media/{id}"));
        if let Some(rg) = range {
            b = b.header("Range", rg);
        }
        if let Some(tag) = inm {
            b = b.header("If-None-Match", tag);
        }
        b.body(Body::empty()).unwrap()
    };

    // Full GET advertises range support + a strong ETag.
    let r = app.clone().oneshot(get(None, None)).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers().get("accept-ranges").unwrap(), "bytes");
    let etag = r
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(etag, format!("\"{id}\""));

    // Satisfiable range -> 206 + Content-Range + exact slice.
    let r = app
        .clone()
        .oneshot(get(Some("bytes=2-5"), None))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(r.headers().get("content-range").unwrap(), "bytes 2-5/26");
    let part = axum::body::to_bytes(r.into_body(), 1_000_000)
        .await
        .unwrap();
    assert_eq!(part.as_ref(), b"CDEF");

    // Suffix range (last 3 bytes) -> 206.
    let r = app
        .clone()
        .oneshot(get(Some("bytes=-3"), None))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::PARTIAL_CONTENT);
    let part = axum::body::to_bytes(r.into_body(), 1_000_000)
        .await
        .unwrap();
    assert_eq!(part.as_ref(), b"XYZ");

    // Unsatisfiable range -> 416 + Content-Range: bytes */len.
    let r = app
        .clone()
        .oneshot(get(Some("bytes=100-200"), None))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(r.headers().get("content-range").unwrap(), "bytes */26");

    // Conditional GET with the current ETag -> 304.
    let r = app.clone().oneshot(get(None, Some(&etag))).await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_MODIFIED);

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
    let state = Arc::new(MediaState::new(
        pool.clone(),
        Arc::new(LocalStorage::new(&dir)),
        tx,
    ));
    state.init().await.unwrap();
    let app = media_router(state, auth);
    std::env::remove_var("STORAGE_MAX_FILE_SIZE");

    let r = app
        .oneshot(upload_req(
            Some(&token),
            "image/png",
            b"way-too-large-payload",
        ))
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
    let state = Arc::new(MediaState::new(
        pool.clone(),
        Arc::new(LocalStorage::new(&dir)),
        tx,
    ));
    state.init().await.unwrap();
    let app = media_router(state, auth);
    std::env::remove_var("STORAGE_PRIVATE_READS");

    // upload as tenant t1 — unique PNG (valid signature + random tail) so dedup doesn't reuse a
    // prior run's media whose bytes live in a now-deleted temp dir.
    let mut png = PNG.to_vec();
    png.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    let r = app
        .clone()
        .oneshot(upload_req(Some(&t1), "image/png", &png))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(r.into_body(), 1_000_000)
        .await
        .unwrap();
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
    assert_eq!(
        app.clone().oneshot(get(None)).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.clone().oneshot(get(Some(&t2))).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.oneshot(get(Some(&t1))).await.unwrap().status(),
        StatusCode::OK
    );

    tokio::fs::remove_dir_all(&dir).await.ok();
}

#[tokio::test]
#[ignore]
async fn media_http_dedups_identical_content_per_tenant() {
    let pool = connect().await;
    let auth = HttpAuthService::new("test-secret", pool.clone());
    let t1 = seed_user_token(&pool, &auth, Some("dt1")).await;
    let t2 = seed_user_token(&pool, &auth, Some("dt2")).await;
    let dir = std::env::temp_dir().join(format!("atomo-media-dedup-{}", uuid::Uuid::new_v4()));
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(MediaState::new(
        pool.clone(),
        Arc::new(LocalStorage::new(&dir)),
        tx,
    ));
    state.init().await.unwrap();
    let app = media_router(state, auth);

    // Unique bytes per run so the dedup is about *these* bytes, not leftovers.
    let data = format!("identical-bytes-{}", uuid::Uuid::new_v4()).into_bytes();
    let up = |token: &str, bytes: &[u8]| {
        app.clone()
            .oneshot(upload_req(Some(token), "application/octet-stream", bytes))
    };
    let id_of = |v: &serde_json::Value| v["id"].as_str().unwrap().to_string();

    // First upload returns an id + a 64-hex checksum.
    let r = up(&t1, &data).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let b1 = json_body(r).await;
    let id1 = id_of(&b1);
    let checksum = b1["checksum"].as_str().unwrap();
    assert_eq!(checksum.len(), 64);
    assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));

    // Same tenant + identical bytes → deduped to the same id (nothing re-stored).
    let r = up(&t1, &data).await.unwrap();
    let id2 = id_of(&json_body(r).await);
    assert_eq!(id1, id2, "identical content for one tenant dedups");

    // A different tenant uploading the same bytes gets its OWN media (no cross-tenant sharing).
    let r = up(&t2, &data).await.unwrap();
    let id3 = id_of(&json_body(r).await);
    assert_ne!(id1, id3, "dedup must be tenant-scoped");

    // Different bytes → a new id even for the same tenant.
    let other = format!("other-bytes-{}", uuid::Uuid::new_v4()).into_bytes();
    let r = up(&t1, &other).await.unwrap();
    assert_ne!(
        id1,
        id_of(&json_body(r).await),
        "different content is a new media"
    );

    tokio::fs::remove_dir_all(&dir).await.ok();
}
