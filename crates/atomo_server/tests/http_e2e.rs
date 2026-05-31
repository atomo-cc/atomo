//! HTTP-layer end-to-end integration tests.
//! Requires a running Postgres instance via DATABASE_URL.
//! Run with: cargo test -p atomo_server --test http_e2e -- --ignored

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

async fn build_app() -> (axum::Router, atomo_server::auth::HttpAuthService) {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let schema_ts = r#"
export interface Note {
  id: string;
  title: string;
}
export const schema = { models: { Note: { tableName: 'notes', access: { read: 'authenticated', create: 'admin', update: 'admin', delete: 'admin' } } } };
export default schema;
"#;
    let atomo = atomo::Atomo::builder()
        .schema_content(schema_ts)
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

    // Seed admin user
    let id = atomo_core::types::EntityId::new().to_string();
    let hash = auth.hash_password("admin123").unwrap();
    sqlx::query(
        "INSERT INTO users (id,email,password_hash,first_name,last_name,role,is_active) \
         VALUES ($1,'admin@test.dev',$2,'A','D','admin',true) ON CONFLICT (email) DO NOTHING",
    )
    .bind(&id)
    .bind(&hash)
    .execute(atomo.db_pool())
    .await
    .unwrap();

    let app = atomo_server::handlers::create_router(gql, atomo, auth.clone(), audit);
    (app, auth)
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
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
async fn test_health_ok() {
    let (app, _) = build_app().await;
    let req = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("OK") || body.contains("ok") || body.contains("healthy"));
}

#[tokio::test]
#[ignore]
async fn test_graphql_requires_auth() {
    let (app, _) = build_app().await;
    let req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"query":"{ __typename }"}"#))
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore]
async fn test_login_then_create_and_list() {
    let (app, _) = build_app().await;

    // 1. Login
    let login_req = Request::builder()
        .uri("/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"admin@test.dev","password":"admin123"}"#,
        ))
        .unwrap();
    let (status, login_json) = send(&app, login_req).await;
    assert_eq!(status, StatusCode::OK, "login failed: {:?}", login_json);
    let token = login_json["token"]
        .as_str()
        .expect("no token in login response");

    // 2. Create a Note via GraphQL
    let create_body = serde_json::json!({
        "query": r#"mutation { create(model: "Note", data: { title: "hello" }) }"#
    });
    let create_req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
        .unwrap();
    let (status, create_json) = send(&app, create_req).await;
    assert_eq!(status, StatusCode::OK, "create failed: {:?}", create_json);
    assert!(
        create_json.get("errors").is_none()
            || create_json["errors"]
                .as_array()
                .map_or(true, |a| a.is_empty()),
        "GraphQL errors on create: {:?}",
        create_json
    );

    // 3. List Notes
    let list_body = serde_json::json!({
        "query": r#"{ records(model: "Note") }"#
    });
    let list_req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&list_body).unwrap()))
        .unwrap();
    let (status, list_json) = send(&app, list_req).await;
    assert_eq!(status, StatusCode::OK, "list failed: {:?}", list_json);
    let list_str = serde_json::to_string(&list_json).unwrap();
    assert!(
        list_str.contains("hello"),
        "created note not found in list: {:?}",
        list_json
    );
}

#[tokio::test]
#[ignore]
async fn test_workflow_register_list_run_delete() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    // Pool-backed engine to also exercise persistence (init + upsert + delete).
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let engine = std::sync::Arc::new(atomo::workflow::WorkflowEngine::with_pool(pool));
    engine.init().await.unwrap();
    let app = atomo_server::handlers::workflow_router(engine.clone());

    // 1. Register a Manual workflow
    let wf = serde_json::json!({
        "name": "http-test-wf",
        "trigger": "Manual",
        "steps": [
            { "name": "flag", "action": { "SetVariable": { "key": "done", "value": true } },
              "condition": null, "on_failure": "Continue" }
        ]
    });
    let req = Request::builder()
        .uri("/workflows")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&wf).unwrap()))
        .unwrap();
    let (status, json) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "register failed: {:?}", json);
    assert_eq!(json["registered"], "http-test-wf");

    // 2. List includes it
    let req = Request::builder()
        .uri("/workflows")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let (status, json) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(serde_json::to_string(&json)
        .unwrap()
        .contains("http-test-wf"));

    // 3. Run it
    let req = Request::builder()
        .uri("/workflows/http-test-wf/run")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let (status, json) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "run failed: {:?}", json);
    assert_eq!(json["status"], "Completed");

    // 4. Persistence: a fresh engine on the same DB loads it
    let pool2 = sqlx::PgPool::connect(&url).await.unwrap();
    let engine2 = atomo::workflow::WorkflowEngine::with_pool(pool2);
    engine2.init().await.unwrap();
    assert!(
        engine2.list().contains(&"http-test-wf".to_string()),
        "workflow did not persist across engines"
    );

    // 5. Delete it
    let req = Request::builder()
        .uri("/workflows/http-test-wf")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();
    let (status, json) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "delete failed: {:?}", json);
    assert_eq!(json["removed"], "http-test-wf");

    // 6. Deleting again is 404
    let req = Request::builder()
        .uri("/workflows/http-test-wf")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
