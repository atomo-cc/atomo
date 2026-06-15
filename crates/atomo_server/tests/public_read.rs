//! HTTP integration tests for the generic public-read route `GET /public/records/{model}`
//! (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test public_read -- --ignored --test-threads=1
//!
//! Proves the AT2 policy end to end: default deny for non-allowlisted models, operator-declared
//! fixed filters (no hardcoded `status`/`slug`), only operator-approved query fields honored, and a
//! client cannot widen the exposed rows.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

const SCHEMA: &str = r#"
import { model, text, allow } from '@atomo/schema'
export const PubDoc = model('at2_pub_docs', {
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

async fn build_app() -> axum::Router {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let atomo = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();

    // Fresh rows each run: two published, one draft.
    sqlx::query("DELETE FROM at2_pub_docs")
        .execute(atomo.db_pool())
        .await
        .unwrap();
    for (id, slug, status) in [
        ("d1", "logo-maker", "published"),
        ("d2", "photo-editor", "published"),
        ("d3", "secret-draft", "draft"),
    ] {
        sqlx::query("INSERT INTO at2_pub_docs (id, slug, status, title) VALUES ($1,$2,$3,$4)")
            .bind(id)
            .bind(slug)
            .bind(status)
            .bind(format!("Title {id}"))
            .execute(atomo.db_pool())
            .await
            .unwrap();
    }

    let gql = atomo_server::handlers::build_extended_schema(&atomo);
    let auth = atomo_server::auth::HttpAuthService::new("test-secret", atomo.db_pool().clone());
    let audit = atomo_server::audit::HttpAuditService::new(atomo.db_pool().clone());
    atomo_server::ensure_platform_tables(atomo.db_pool())
        .await
        .unwrap();
    atomo_server::handlers::create_router(
        gql,
        atomo,
        auth,
        audit,
        atomo_server::auth::RegistrationConfig::disabled(),
    )
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .uri(uri)
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn slugs(body: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = body["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|r| r.get("slug").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

#[tokio::test]
#[ignore]
async fn public_read_enforces_dual_approval_filters_and_query_fields() {
    // Operator config: allowlist the model, force status=published, allow filtering only by slug.
    std::env::set_var("ATOMO_PUBLIC_READ_MODELS", "PubDoc");
    std::env::set_var("ATOMO_PUBLIC_READ_FILTER_PubDoc", "status:published");
    std::env::set_var("ATOMO_PUBLIC_READ_FIELDS_PubDoc", "slug");

    let app = build_app().await;

    // Fixed filter applied: only published rows, never the draft.
    let (status, body) = get(&app, "/public/records/PubDoc").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(slugs(&body), vec!["logo-maker", "photo-editor"]);

    // Allowed query field narrows to one row.
    let (_, body) = get(&app, "/public/records/PubDoc?slug=logo-maker").await;
    assert_eq!(slugs(&body), vec!["logo-maker"]);

    // A client cannot reach the draft by asking for it (status is not an allowed query field, and
    // the fixed filter wins regardless).
    let (_, body) = get(
        &app,
        "/public/records/PubDoc?status=draft&slug=secret-draft",
    )
    .await;
    assert!(
        slugs(&body).is_empty(),
        "client must not be able to widen rows beyond the fixed filter: {body}"
    );

    // A non-allowlisted model is denied (default deny).
    let (status, _) = get(&app, "/public/records/Other").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    std::env::remove_var("ATOMO_PUBLIC_READ_MODELS");
    std::env::remove_var("ATOMO_PUBLIC_READ_FILTER_PubDoc");
    std::env::remove_var("ATOMO_PUBLIC_READ_FIELDS_PubDoc");
}

#[tokio::test]
#[ignore]
async fn public_read_denies_model_not_in_allowlist_even_if_public() {
    // The model declares allow.public() but the operator did NOT allowlist it -> 404.
    std::env::remove_var("ATOMO_PUBLIC_READ_MODELS");
    let app = build_app().await;
    let (status, _) = get(&app, "/public/records/PubDoc").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
