//! Integration tests for the generic public-read redirect store.
//! Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test public_read_redirects -- --ignored --test-threads=1

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

const SCHEMA: &str = r#"
import { model, text, allow } from '@atomo/schema'
export const Article = model('pr_articles', {
  fields: {
    id: text().id(),
    slug: text().required(),
    status: text().required(),
    title: text().required(),
  },
  access: {
    read: allow.public(),
    create: allow.role('admin'),
    update: allow.role('admin'),
    delete: allow.role('admin'),
  },
})
"#;

async fn build_app() -> (axum::Router, sqlx::PgPool) {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let atomo = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let pool = atomo.db_pool().clone();

    sqlx::query("DELETE FROM pr_articles")
        .execute(&pool)
        .await
        .unwrap();
    for (id, slug, status) in [
        ("a1", "old-slug", "published"),
        ("a2", "current-slug", "published"),
    ] {
        sqlx::query("INSERT INTO pr_articles (id, slug, status, title) VALUES ($1,$2,$3,$4)")
            .bind(id)
            .bind(slug)
            .bind(status)
            .bind(format!("Title {id}"))
            .execute(&pool)
            .await
            .unwrap();
    }

    std::env::set_var("ATOMO_PUBLIC_READ_MODELS", "Article");
    std::env::set_var("ATOMO_PUBLIC_READ_FIELDS_Article", "slug");
    std::env::set_var("ATOMO_PUBLIC_READ_FILTER_Article", "status:published");

    let gql = atomo_server::handlers::build_extended_schema(&atomo);
    let auth = atomo_server::auth::HttpAuthService::new("test-secret", pool.clone());
    let audit = atomo_server::audit::HttpAuditService::new(pool.clone());
    atomo_server::ensure_platform_tables(&pool).await.unwrap();

    let redirect_store = std::sync::Arc::new(
        atomo_server::public_read_redirects::RedirectStore::new(pool.clone()),
    );
    redirect_store.init().await.unwrap();

    let app = atomo_server::handlers::create_router(
        gql,
        atomo,
        auth,
        audit,
        atomo_server::auth::RegistrationConfig::disabled(),
        vec!["Article".to_string()],
        redirect_store,
    );
    (app, pool)
}

async fn get_raw(app: &axum::Router, uri: &str) -> axum::http::Response<Body> {
    let req = Request::builder()
        .uri(uri)
        .method("GET")
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap()
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let resp = get_raw(app, uri).await;
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
#[ignore]
async fn redirect_on_missing_slug() {
    let (app, pool) = build_app().await;

    // Normal lookup works.
    let (status, body) = get(&app, "/public/records/Article?slug=current-slug").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // Insert a redirect: renamed-slug → current-slug (permanent).
    let store = atomo_server::public_read_redirects::RedirectStore::new(pool.clone());
    store
        .upsert("Article", "renamed-slug", "current-slug", true)
        .await
        .unwrap();

    // Query for "renamed-slug" should 301 to current-slug.
    let resp = get_raw(&app, "/public/records/Article?slug=renamed-slug").await;
    assert_eq!(resp.status(), StatusCode::MOVED_PERMANENTLY);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        location.contains("slug=current-slug"),
        "redirect location should point to current-slug, got: {location}"
    );

    // Cleanup env vars.
    std::env::remove_var("ATOMO_PUBLIC_READ_MODELS");
    std::env::remove_var("ATOMO_PUBLIC_READ_FIELDS_Article");
    std::env::remove_var("ATOMO_PUBLIC_READ_FILTER_Article");
}

#[tokio::test]
#[ignore]
async fn temporary_redirect_returns_302() {
    let (app, pool) = build_app().await;

    let store = atomo_server::public_read_redirects::RedirectStore::new(pool.clone());
    store
        .upsert("Article", "temp-slug", "current-slug", false)
        .await
        .unwrap();

    let resp = get_raw(&app, "/public/records/Article?slug=temp-slug").await;
    assert_eq!(resp.status(), StatusCode::FOUND);

    std::env::remove_var("ATOMO_PUBLIC_READ_MODELS");
    std::env::remove_var("ATOMO_PUBLIC_READ_FIELDS_Article");
    std::env::remove_var("ATOMO_PUBLIC_READ_FILTER_Article");
}

#[tokio::test]
#[ignore]
async fn no_redirect_without_slug_param() {
    let (app, pool) = build_app().await;

    let store = atomo_server::public_read_redirects::RedirectStore::new(pool.clone());
    store
        .upsert("Article", "some-slug", "current-slug", true)
        .await
        .unwrap();

    // Query without slug param returns all rows, no redirect.
    let (status, body) = get(&app, "/public/records/Article").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["items"].as_array().unwrap().len() >= 1);

    std::env::remove_var("ATOMO_PUBLIC_READ_MODELS");
    std::env::remove_var("ATOMO_PUBLIC_READ_FIELDS_Article");
    std::env::remove_var("ATOMO_PUBLIC_READ_FILTER_Article");
}

#[tokio::test]
#[ignore]
async fn store_upsert_remove_list() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let store = atomo_server::public_read_redirects::RedirectStore::new(pool.clone());
    store.init().await.unwrap();

    // Clean slate.
    sqlx::query("DELETE FROM public_read_redirects WHERE model = 'StoreTest'")
        .execute(&pool)
        .await
        .unwrap();

    // Upsert + lookup.
    store.upsert("StoreTest", "a", "b", true).await.unwrap();
    let r = store.lookup("StoreTest", "a").await.unwrap().unwrap();
    assert_eq!(r.to_slug, "b");
    assert!(r.permanent);

    // Upsert overwrites.
    store.upsert("StoreTest", "a", "c", false).await.unwrap();
    let r = store.lookup("StoreTest", "a").await.unwrap().unwrap();
    assert_eq!(r.to_slug, "c");
    assert!(!r.permanent);

    // List.
    store.upsert("StoreTest", "d", "e", true).await.unwrap();
    let list = store.list("StoreTest").await.unwrap();
    assert_eq!(list.len(), 2);

    // Remove.
    assert!(store.remove("StoreTest", "a").await.unwrap());
    assert!(!store.remove("StoreTest", "a").await.unwrap());
    assert!(store.lookup("StoreTest", "a").await.unwrap().is_none());
}
