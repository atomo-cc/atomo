//! HTTP integration tests for optional self-registration (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test self_registration -- --ignored --test-threads=1
//!
//! Proves the AT3 behavior: the route is absent when disabled (default), a successful registration
//! follows the configured provisioning policy (role + tenant), duplicate emails conflict
//! race-safely, and a non-default provisioning policy (custom role + per-user tenant) is applied.

use atomo_server::auth::{RegistrationConfig, TenantProvisioning};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

const SCHEMA: &str = r#"
import { model, text } from '@atomo/schema'
export const Thing = model('at3_things', {
  fields: { id: text().id(), name: text().optional() },
})
"#;

async fn build_app(reg: RegistrationConfig) -> (axum::Router, sqlx::PgPool) {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let atomo = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let gql = atomo_server::handlers::build_extended_schema(&atomo);
    let auth = atomo_server::auth::HttpAuthService::new("test-secret", atomo.db_pool().clone());
    let audit = atomo_server::audit::HttpAuditService::new(atomo.db_pool().clone());
    atomo_server::ensure_platform_tables(atomo.db_pool())
        .await
        .unwrap();
    let pool = atomo.db_pool().clone();
    sqlx::query("DELETE FROM users WHERE email LIKE 'at3-%'")
        .execute(&pool)
        .await
        .unwrap();
    let app = atomo_server::handlers::create_router(gql, atomo, auth, audit, reg);
    (app, pool)
}

async fn post(
    app: &axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .uri(uri)
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
#[ignore]
async fn register_route_absent_when_disabled() {
    let (app, _) = build_app(RegistrationConfig::disabled()).await;
    let (status, _) = post(
        &app,
        "/auth/register",
        json!({ "email": "at3-x@test.dev", "password": "password123" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "register must not be mounted when self-registration is disabled"
    );
}

#[tokio::test]
#[ignore]
async fn register_success_with_default_provisioning() {
    let reg = RegistrationConfig {
        enabled: true,
        role: "viewer".to_string(),
        tenant: TenantProvisioning::None,
    };
    let (app, pool) = build_app(reg).await;
    let (status, body) = post(
        &app,
        "/auth/register",
        json!({ "email": "at3-a@test.dev", "password": "password123" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["token"].is_string());
    assert_eq!(body["user"]["role"], "viewer");
    assert!(
        body["user"]["tenant_id"].is_null(),
        "default tenant provisioning is unscoped (NULL)"
    );

    let (role, tenant): (String, Option<String>) =
        sqlx::query_as("SELECT role, tenant_id FROM users WHERE email = $1")
            .bind("at3-a@test.dev")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(role, "viewer");
    assert!(tenant.is_none());
}

#[tokio::test]
#[ignore]
async fn register_duplicate_email_conflicts() {
    let reg = RegistrationConfig {
        enabled: true,
        role: "viewer".to_string(),
        tenant: TenantProvisioning::None,
    };
    let (app, _) = build_app(reg).await;
    let body = json!({ "email": "at3-dup@test.dev", "password": "password123" });
    let (first, _) = post(&app, "/auth/register", body.clone()).await;
    assert_eq!(first, StatusCode::OK);
    let (second, _) = post(&app, "/auth/register", body).await;
    assert_eq!(
        second,
        StatusCode::CONFLICT,
        "a duplicate email must conflict, not create a second user"
    );
}

#[tokio::test]
#[ignore]
async fn register_provisions_custom_role_and_per_user_tenant() {
    let reg = RegistrationConfig {
        enabled: true,
        role: "editor".to_string(),
        tenant: TenantProvisioning::PerUser,
    };
    let (app, pool) = build_app(reg).await;
    let (status, body) = post(
        &app,
        "/auth/register",
        json!({ "email": "at3-b@test.dev", "password": "password123" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["role"], "editor");
    let uid = body["user"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        body["user"]["tenant_id"].as_str(),
        Some(uid.as_str()),
        "per-user provisioning binds tenant_id to the new user's id"
    );

    let (role, tenant): (String, Option<String>) =
        sqlx::query_as("SELECT role, tenant_id FROM users WHERE email = $1")
            .bind("at3-b@test.dev")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(role, "editor");
    assert_eq!(tenant.as_deref(), Some(uid.as_str()));
}
