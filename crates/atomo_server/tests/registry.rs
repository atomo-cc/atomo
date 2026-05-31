//! Registry read-API integration tests (marketplace milestone 1).
//! Requires Postgres via DATABASE_URL.
//! Run: cargo test -p atomo_server --test registry -- --ignored

use atomo_server::registry::RegistryStore;
use atomo_server::registry_routes::registry_router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

async fn setup() -> (axum::Router, std::path::PathBuf) {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let blob_dir = std::env::temp_dir().join(format!(
        "atomo_reg_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = RegistryStore::new(pool.clone(), &blob_dir);
    store.init().await.unwrap();

    // Seed a plugin + version + artifact blob.
    sqlx::query("INSERT INTO plugins (name, description, author, latest_version) VALUES ('enrich','Enrich contacts','acme','0.1.0') ON CONFLICT (name) DO UPDATE SET latest_version='0.1.0'")
        .execute(&pool).await.unwrap();
    tokio::fs::write(blob_dir.join("enrich-0.1.0.wasm"), b"\0asm\x01\0\0\0")
        .await
        .unwrap();
    sqlx::query("INSERT INTO plugin_versions (name, version, checksum, manifest, artifact_path) VALUES ('enrich','0.1.0','sha256:abc','{\"permissions\":[\"ReadEvents\"]}','enrich-0.1.0.wasm') ON CONFLICT (name, version) DO NOTHING")
        .execute(&pool).await.unwrap();

    (registry_router(Arc::new(store)), blob_dir)
}

async fn send(app: &axum::Router, uri: &str) -> (StatusCode, Vec<u8>) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    (status, bytes.to_vec())
}

#[tokio::test]
#[ignore]
async fn test_registry_search_get_download() {
    let (app, dir) = setup().await;

    // search (no query lists all; includes our seeded plugin)
    let (status, body) = send(&app, "/registry/plugins").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let plugins = json["plugins"].as_array().unwrap();
    assert!(
        plugins.iter().any(|p| p["name"] == "enrich"),
        "search missing seeded plugin: {:?}",
        json
    );

    // search with a matching query
    let (status, body) = send(&app, "/registry/plugins?q=enrich").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!json["plugins"].as_array().unwrap().is_empty());

    // get plugin -> metadata + versions
    let (status, body) = send(&app, "/registry/plugins/enrich").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "enrich");
    assert_eq!(json["latest_version"], "0.1.0");
    assert_eq!(json["versions"].as_array().unwrap()[0]["version"], "0.1.0");

    // get unknown plugin -> 404
    let (status, _) = send(&app, "/registry/plugins/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // download artifact -> wasm bytes
    let (status, body) = send(&app, "/registry/plugins/enrich/0.1.0/download").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&body[..4], b"\0asm", "expected wasm magic bytes");

    // download unknown version -> 404
    let (status, _) = send(&app, "/registry/plugins/enrich/9.9.9/download").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    tokio::fs::remove_dir_all(&dir).await.ok();
}
