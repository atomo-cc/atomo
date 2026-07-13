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
    let redirect_store = std::sync::Arc::new(
        atomo_server::public_read_redirects::RedirectStore::new(atomo.db_pool().clone()),
    );
    redirect_store.init().await.unwrap();
    // Mirror server boot: the allowlist param comes from ATOMO_PUBLIC_READ_MODELS.
    // Tests set that env per-case (PubDoc); a hardcoded list here silently 404s
    // every allowlisted request.
    let public_models: Vec<String> = std::env::var("ATOMO_PUBLIC_READ_MODELS")
        .map(|s| s.split(',').map(|m| m.trim().to_string()).collect())
        .unwrap_or_default();
    atomo_server::handlers::create_router(
        gql,
        atomo,
        auth,
        audit,
        atomo_server::auth::RegistrationConfig::disabled(),
        public_models,
        redirect_store,
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

#[tokio::test]
#[ignore]
async fn public_read_limit_clamped_to_100() {
    std::env::set_var("ATOMO_PUBLIC_READ_MODELS", "PubDoc");
    std::env::set_var("ATOMO_PUBLIC_READ_FILTER_PubDoc", "status:published");
    std::env::set_var("ATOMO_PUBLIC_READ_FIELDS_PubDoc", "slug");
    let app = build_app().await;

    // limit=0 should clamp to 1 (minimum), not return zero rows or error.
    let (status, body) = get(&app, "/public/records/PubDoc?limit=0").await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "limit=0 should clamp to 1");

    // limit=999 should clamp to 100 (the maximum).
    let (status, _) = get(&app, "/public/records/PubDoc?limit=999").await;
    assert_eq!(status, StatusCode::OK);

    // Negative limit falls back to default (non-parseable).
    let (status, body) = get(&app, "/public/records/PubDoc?limit=-5").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["items"].as_array().unwrap().len() <= 100);

    std::env::remove_var("ATOMO_PUBLIC_READ_MODELS");
    std::env::remove_var("ATOMO_PUBLIC_READ_FILTER_PubDoc");
    std::env::remove_var("ATOMO_PUBLIC_READ_FIELDS_PubDoc");
}

#[tokio::test]
#[ignore]
async fn public_read_no_filter_config_exposes_all_rows() {
    // Allowlisted but no fixed filter and no query fields — all rows visible, no client filtering.
    std::env::set_var("ATOMO_PUBLIC_READ_MODELS", "PubDoc");
    std::env::remove_var("ATOMO_PUBLIC_READ_FILTER_PubDoc");
    std::env::remove_var("ATOMO_PUBLIC_READ_FIELDS_PubDoc");
    let app = build_app().await;

    let (status, body) = get(&app, "/public/records/PubDoc").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        slugs(&body),
        vec!["logo-maker", "photo-editor", "secret-draft"],
        "without fixed filter, all rows (including draft) should be exposed"
    );

    // Client filter param is ignored when no query fields are configured.
    let (_, body) = get(&app, "/public/records/PubDoc?slug=logo-maker").await;
    assert_eq!(
        slugs(&body).len(),
        3,
        "slug param should be ignored when ATOMO_PUBLIC_READ_FIELDS is not set"
    );

    std::env::remove_var("ATOMO_PUBLIC_READ_MODELS");
}

#[tokio::test]
#[ignore]
async fn public_read_fixed_filter_overrides_client_attempt() {
    // Even when a field appears in both the fixed filter and allowed query fields, the fixed value
    // wins — a client can never override the operator's forced filter.
    std::env::set_var("ATOMO_PUBLIC_READ_MODELS", "PubDoc");
    std::env::set_var("ATOMO_PUBLIC_READ_FILTER_PubDoc", "status:published");
    std::env::set_var("ATOMO_PUBLIC_READ_FIELDS_PubDoc", "slug,status");
    let app = build_app().await;

    let (_, body) = get(&app, "/public/records/PubDoc?status=draft").await;
    let s = slugs(&body);
    assert!(
        !s.contains(&"secret-draft".to_string()),
        "fixed filter must override client's status=draft attempt: got {s:?}"
    );
    assert_eq!(s, vec!["logo-maker", "photo-editor"]);

    std::env::remove_var("ATOMO_PUBLIC_READ_MODELS");
    std::env::remove_var("ATOMO_PUBLIC_READ_FILTER_PubDoc");
    std::env::remove_var("ATOMO_PUBLIC_READ_FIELDS_PubDoc");
}
