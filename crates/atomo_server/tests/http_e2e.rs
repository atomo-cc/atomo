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

    // Seed a viewer user for RBAC tests (Note.create requires admin).
    let vid = atomo_core::types::EntityId::new().to_string();
    let vhash = auth.hash_password("viewer123").unwrap();
    sqlx::query(
        "INSERT INTO users (id,email,password_hash,first_name,last_name,role,is_active) \
         VALUES ($1,'viewer@test.dev',$2,'V','R','viewer',true) ON CONFLICT (email) DO NOTHING",
    )
    .bind(&vid)
    .bind(&vhash)
    .execute(atomo.db_pool())
    .await
    .unwrap();

    let app = atomo_server::handlers::create_router(
        gql,
        atomo,
        auth.clone(),
        audit,
        atomo_server::auth::RegistrationConfig::disabled(),
        vec![],
    );
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
                .is_none_or(|a| a.is_empty()),
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

// B1: a model mutation produces an audit_log entry, attributed to the actor.
#[tokio::test]
#[ignore]
async fn test_mutation_writes_audit_entry_with_actor() {
    use atomo_core::audit::{AuditLogEntry, AuditOperation, AuditService};
    use atomo_core::types::EntityId;
    use std::collections::HashMap;

    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let schema_ts = r#"
export interface Note {
  id: string;
  title: string;
}
export const schema = { models: { Note: { tableName: 'notes' } } };
export default schema;
"#;
    let atomo = atomo::Atomo::builder()
        .schema_content(schema_ts)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let audit = atomo_server::audit::HttpAuditService::new(atomo.db_pool().clone());
    atomo_server::ensure_platform_tables(atomo.db_pool())
        .await
        .unwrap();

    // Wire the same audit listener the server boot uses.
    {
        let audit = audit.clone();
        let mut rx = atomo.event_receiver();
        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                let op = match ev.event_type {
                    atomo::events::EventType::Created => AuditOperation::Create,
                    atomo::events::EventType::Updated
                    | atomo::events::EventType::Restored => AuditOperation::Update,
                    atomo::events::EventType::Deleted
                    | atomo::events::EventType::HardDeleted => AuditOperation::Delete,
                    atomo::events::EventType::Custom => AuditOperation::Read,
                };
                let entity_id = ev
                    .data
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| EntityId::from_string(s).ok())
                    .unwrap_or_else(EntityId::new);
                let details = serde_json::to_string(&ev.data).unwrap_or_default();
                let entry = AuditLogEntry::new(
                    ev.model_name.clone(),
                    entity_id,
                    op,
                    details,
                    ev.actor.clone(),
                );
                let _ = audit.log_audit_entry(entry).await;
            }
        });
    }

    // Create a Note with an explicit actor.
    let mut data = HashMap::new();
    data.insert("title".to_string(), serde_json::json!("audit me"));
    atomo
        .client()
        .create("Note", &data, &[], Some("user-42"))
        .await
        .unwrap();

    // Give the listener a moment to persist.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let row: (String, i16, Option<String>) = sqlx::query_as(
        "SELECT entity_type, operation, user_id FROM audit_log WHERE entity_type = 'Note' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(atomo.db_pool())
    .await
    .expect("no audit row written");

    assert_eq!(row.0, "Note");
    assert_eq!(row.1, AuditOperation::Create as i16);
    assert_eq!(
        row.2.as_deref(),
        Some("user-42"),
        "audit entry should capture the actor"
    );
}

// Regression: delete must honor its `where` filter and NOT affect other records.
#[tokio::test]
#[ignore]
async fn test_delete_respects_where_filter() {
    let (app, _) = build_app().await;

    let login_req = Request::builder()
        .uri("/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"admin@test.dev","password":"admin123"}"#,
        ))
        .unwrap();
    let (_, login_json) = send(&app, login_req).await;
    let token = login_json["token"].as_str().expect("no token");

    // Create two notes.
    for title in ["keep", "remove"] {
        let body = serde_json::json!({
            "query": format!(r#"mutation {{ create(model: "Note", data: {{ title: "{}" }}) }}"#, title)
        });
        let req = Request::builder()
            .uri("/graphql")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let (status, j) = send(&app, req).await;
        assert_eq!(status, StatusCode::OK, "create failed: {:?}", j);
    }

    // Find the id of the "remove" note.
    let list_body = serde_json::json!({ "query": r#"{ records(model: "Note") }"# });
    let req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&list_body).unwrap()))
        .unwrap();
    let (_, list_json) = send(&app, req).await;
    let records = list_json["data"]["records"]
        .as_array()
        .expect("records array");
    let remove_id = records
        .iter()
        .find(|r| r["title"] == "remove")
        .and_then(|r| r["id"].as_str())
        .expect("remove note not found")
        .to_string();

    // Delete ONLY the "remove" note by id.
    let del_body = serde_json::json!({
        "query": "mutation($w: JSON!){ delete(model: \"Note\", where: $w) }",
        "variables": { "w": { "id": { "equals": remove_id } } }
    });
    let req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&del_body).unwrap()))
        .unwrap();
    let (status, del_json) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "delete failed: {:?}", del_json);
    assert_eq!(
        del_json["data"]["delete"], 1,
        "delete should affect exactly 1 row, got: {:?}",
        del_json
    );

    // The "keep" note must still be present.
    let req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&list_body).unwrap()))
        .unwrap();
    let (_, after_json) = send(&app, req).await;
    let after = after_json["data"]["records"]
        .as_array()
        .expect("records array");
    assert!(
        after.iter().any(|r| r["title"] == "keep"),
        "the non-matching record was wrongly deleted"
    );
    assert!(
        !after.iter().any(|r| r["title"] == "remove"),
        "the matching record was not deleted"
    );
}

// Regression: paginatedRecords must accept and apply a `where` filter (admin list view).
#[tokio::test]
#[ignore]
async fn test_paginated_records_with_where_filter() {
    let (app, _) = build_app().await;

    let login_req = Request::builder()
        .uri("/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"admin@test.dev","password":"admin123"}"#,
        ))
        .unwrap();
    let (_, login_json) = send(&app, login_req).await;
    let token = login_json["token"].as_str().expect("no token");

    // Create notes with distinct titles.
    for title in ["alpha", "beta", "alpha"] {
        let body = serde_json::json!({
            "query": format!(r#"mutation {{ create(model: "Note", data: {{ title: "{}" }}) }}"#, title)
        });
        let req = Request::builder()
            .uri("/graphql")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let (status, _) = send(&app, req).await;
        assert_eq!(status, StatusCode::OK);
    }

    // paginatedRecords filtered to title = "beta" should return exactly 1.
    let body = serde_json::json!({
        "query": "query($w: JSON){ paginatedRecords(model: \"Note\", where: $w) { data pageInfo { totalCount } } }",
        "variables": { "w": { "title": { "equals": "beta" } } }
    });
    let req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let (status, json) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "query failed: {:?}", json);
    assert!(
        json.get("errors").is_none() || json["errors"].as_array().is_none_or(|a| a.is_empty()),
        "GraphQL errors: {:?}",
        json
    );
    let page = &json["data"]["paginatedRecords"];
    let data = page["data"].as_array().expect("data array");
    assert_eq!(
        data.len(),
        1,
        "where filter should return exactly the matching record"
    );
    assert_eq!(data[0]["title"], "beta");
    assert_eq!(
        page["pageInfo"]["totalCount"], 1,
        "totalCount should reflect the filter"
    );
}

// Update must apply data to only the where-matched record and persist the change.
#[tokio::test]
#[ignore]
async fn test_update_scoped_and_persists() {
    let (app, _) = build_app().await;

    let login_req = Request::builder()
        .uri("/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"admin@test.dev","password":"admin123"}"#,
        ))
        .unwrap();
    let (_, login_json) = send(&app, login_req).await;
    let token = login_json["token"].as_str().expect("no token");

    let create = |title: &str| {
        let body = serde_json::json!({
            "query": format!(r#"mutation {{ create(model: "Note", data: {{ title: "{}" }}) }}"#, title)
        });
        Request::builder()
            .uri("/graphql")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    };
    let (_, c1) = send(&app, create("orig")).await;
    let target_id = c1["data"]["create"]["id"].as_str().expect("id").to_string();
    send(&app, create("untouched")).await;

    // Update only the target note's title.
    let upd = serde_json::json!({
        "query": "mutation($w: JSON!, $d: JSON!){ update(model: \"Note\", where: $w, data: $d) }",
        "variables": { "w": { "id": { "equals": target_id } }, "d": { "title": "changed" } }
    });
    let req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&upd).unwrap()))
        .unwrap();
    let (status, upd_json) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "update failed: {:?}", upd_json);
    assert!(
        upd_json.get("errors").is_none()
            || upd_json["errors"].as_array().is_none_or(|a| a.is_empty()),
        "GraphQL errors on update: {:?}",
        upd_json
    );

    // Read back: target is "changed", the other note is still "untouched".
    let list = serde_json::json!({ "query": r#"{ records(model: "Note") }"# });
    let req = Request::builder()
        .uri("/graphql")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&list).unwrap()))
        .unwrap();
    let (_, list_json) = send(&app, req).await;
    let recs = list_json["data"]["records"].as_array().expect("records");
    assert!(
        recs.iter().any(|r| r["title"] == "changed"),
        "target update did not persist"
    );
    assert!(
        recs.iter().any(|r| r["title"] == "untouched"),
        "non-target was wrongly modified"
    );
    assert!(
        !recs.iter().any(|r| r["title"] == "orig"),
        "old value still present"
    );
}

// Phase S1: RBAC is enforced end-to-end. Note.create requires 'admin' (build_app schema).
// A viewer logging in and attempting create must be denied; admin must succeed.
#[tokio::test]
#[ignore]
async fn test_rbac_viewer_denied_create_admin_allowed() {
    let (app, _) = build_app().await;

    let login = |email: &str, pw: &str| serde_json::json!({ "email": email, "password": pw });
    let do_login = |app: &axum::Router, body: serde_json::Value| {
        let app = app.clone();
        async move {
            let req = Request::builder()
                .uri("/auth/login")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap();
            let (_, json) = send(&app, req).await;
            json["token"].as_str().map(|s| s.to_string())
        }
    };

    let create_req = |token: &str| {
        let body = serde_json::json!({
            "query": r#"mutation { create(model: "Note", data: { title: "rbac" }) }"#
        });
        Request::builder()
            .uri("/graphql")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    };

    // Viewer: denied (Note.create requires admin).
    let vtoken = do_login(&app, login("viewer@test.dev", "viewer123"))
        .await
        .expect("viewer login");
    let (_, vjson) = send(&app, create_req(&vtoken)).await;
    let verrs = serde_json::to_string(&vjson).unwrap();
    assert!(
        vjson
            .get("errors")
            .and_then(|e| e.as_array())
            .is_some_and(|a| !a.is_empty()),
        "viewer create should be denied by RBAC, got: {}",
        verrs
    );
    assert!(
        verrs.contains("denied") || verrs.contains("Access"),
        "denial should cite access: {}",
        verrs
    );

    // Admin: allowed.
    let atoken = do_login(&app, login("admin@test.dev", "admin123"))
        .await
        .expect("admin login");
    let (_, ajson) = send(&app, create_req(&atoken)).await;
    assert!(
        ajson.get("errors").is_none() || ajson["errors"].as_array().is_none_or(|a| a.is_empty()),
        "admin create should be allowed, got: {:?}",
        ajson
    );
}

// Phase S2: the model_changes subscription is gated by read access using the injected role.
// Without a role (the unauth WS case) it must error; with a valid role it must open a stream.
// (The WebSocket connection_init auth that injects the role is exercised at the handler level;
// here we prove the second-layer access gate that closes the bypass.)
#[tokio::test]
#[ignore]
async fn test_subscription_requires_auth_role() {
    use async_graphql::futures_util::StreamExt;
    use atomo::graphql::UserRoleCtx;

    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    // Note has read: 'authenticated' in build_app's schema.
    let schema_ts = r#"
export interface Note {
  id: string;
  title: string;
}
export const schema = { models: { Note: { tableName: 'notes', access: { read: 'authenticated', create: 'admin' } } } };
export default schema;
"#;
    let atomo = atomo::Atomo::builder()
        .schema_content(schema_ts)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let schema = atomo_server::handlers::build_extended_schema(&atomo);

    let sub_query = r#"subscription { modelChanges(model: "Note") { modelName eventType } }"#;

    // No role injected (unauthenticated): the stream's first item must be an error.
    let mut anon = schema.execute_stream(async_graphql::Request::new(sub_query));
    let first = anon.next().await.expect("a response");
    assert!(
        !first.errors.is_empty(),
        "unauthenticated subscription must be rejected"
    );

    // With a role injected (as connection_init would do): no auth error.
    let req = async_graphql::Request::new(sub_query).data(UserRoleCtx("Viewer".to_string()));
    let mut authed = schema.execute_stream(req);
    // The stream stays open (pending) for an authorized subscriber; just assert it didn't
    // immediately yield an auth error. Use a short timeout since no events will arrive.
    let res = tokio::time::timeout(std::time::Duration::from_millis(300), authed.next()).await;
    assert!(
        res.is_err(),
        "authorized subscription should stay open (no immediate error)"
    );
}

// Phase S3: multi-tenant isolation. With the generated tenant_id column, writes are tagged by
// TenantCtx and reads are scoped to it — tenant A and tenant B must not see each other's rows.
#[tokio::test]
#[ignore]
async fn test_two_tenant_isolation() {
    use atomo::graphql::TenantCtx;

    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let schema_ts = r#"
export interface Note {
  id: string;
  title: string;
}
export const schema = { models: { Note: { tableName: 'notes', access: { read: 'public', create: 'public' } } } };
export default schema;
"#;
    let atomo = atomo::Atomo::builder()
        .schema_content(schema_ts)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let schema = atomo_server::handlers::build_extended_schema(&atomo);

    let create = |title: &str| {
        async_graphql::Request::new(format!(
            r#"mutation {{ create(model: "Note", data: {{ title: "{}" }}) }}"#,
            title
        ))
    };
    let list = || async_graphql::Request::new(r#"{ records(model: "Note") }"#);

    // tenant A creates "a-note"; tenant B creates "b-note".
    let ra = schema
        .execute(create("a-note").data(TenantCtx("tenant-a".into())))
        .await;
    assert!(
        ra.errors.is_empty(),
        "tenant A create failed: {:?}",
        ra.errors
    );
    let rb = schema
        .execute(create("b-note").data(TenantCtx("tenant-b".into())))
        .await;
    assert!(
        rb.errors.is_empty(),
        "tenant B create failed: {:?}",
        rb.errors
    );

    // tenant A lists → only a-note.
    let la = schema
        .execute(list().data(TenantCtx("tenant-a".into())))
        .await;
    let la_str = serde_json::to_string(&la.data).unwrap();
    assert!(
        la_str.contains("a-note"),
        "tenant A should see its own note: {}",
        la_str
    );
    assert!(
        !la_str.contains("b-note"),
        "tenant A must NOT see tenant B's note: {}",
        la_str
    );

    // tenant B lists → only b-note.
    let lb = schema
        .execute(list().data(TenantCtx("tenant-b".into())))
        .await;
    let lb_str = serde_json::to_string(&lb.data).unwrap();
    assert!(
        lb_str.contains("b-note"),
        "tenant B should see its own note: {}",
        lb_str
    );
    assert!(
        !lb_str.contains("a-note"),
        "tenant B must NOT see tenant A's note: {}",
        lb_str
    );

    // D1: writes are tenant-scoped too. Tenant B tries to UPDATE every Note title to "hacked"
    // and then DELETE all Notes — both must only touch B's own rows, leaving A's note intact.
    let upd = async_graphql::Request::new(
        r#"mutation { update(model: "Note", where: {}, data: { title: "hacked" }) }"#,
    );
    let _ = schema.execute(upd.data(TenantCtx("tenant-b".into()))).await;
    let del = async_graphql::Request::new(r#"mutation { delete(model: "Note", where: {}) }"#);
    let _ = schema.execute(del.data(TenantCtx("tenant-b".into()))).await;

    // Tenant A still sees its untouched a-note (B's update/delete must not have reached it).
    let la2 = schema
        .execute(list().data(TenantCtx("tenant-a".into())))
        .await;
    let la2_str = serde_json::to_string(&la2.data).unwrap();
    assert!(
        la2_str.contains("a-note"),
        "A's note must survive B's update/delete: {}",
        la2_str
    );
    assert!(
        !la2_str.contains("hacked"),
        "B must NOT have updated A's note: {}",
        la2_str
    );

    sqlx::query("DROP TABLE IF EXISTS notes")
        .execute(atomo.db_pool())
        .await
        .ok();
}

// Phase B4: audit-on-mutation works through the real CRM models (not just a synthetic Note),
// capturing the actor and the correct operation for both create and update.
#[tokio::test]
#[ignore]
async fn test_crm_mutation_audited_with_actor() {
    use atomo_core::audit::{AuditLogEntry, AuditOperation, AuditService};
    use atomo_core::types::EntityId;
    use std::collections::HashMap;

    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../services/crm-service/schema.ts")
        .to_string_lossy()
        .into_owned();
    let atomo = atomo::Atomo::builder()
        .schema_file(schema_path)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let audit = atomo_server::audit::HttpAuditService::new(atomo.db_pool().clone());
    atomo_server::ensure_platform_tables(atomo.db_pool())
        .await
        .unwrap();

    // Same audit listener the server boot wires.
    {
        let audit = audit.clone();
        let mut rx = atomo.event_receiver();
        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                let op = match ev.event_type {
                    atomo::events::EventType::Created => AuditOperation::Create,
                    atomo::events::EventType::Updated
                    | atomo::events::EventType::Restored => AuditOperation::Update,
                    atomo::events::EventType::Deleted
                    | atomo::events::EventType::HardDeleted => AuditOperation::Delete,
                    atomo::events::EventType::Custom => AuditOperation::Read,
                };
                let entity_id = ev
                    .data
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| EntityId::from_string(s).ok())
                    .unwrap_or_else(EntityId::new);
                let details = serde_json::to_string(&ev.data).unwrap_or_default();
                let entry = AuditLogEntry::new(
                    ev.model_name.clone(),
                    entity_id,
                    op,
                    details,
                    ev.actor.clone(),
                );
                let _ = audit.log_audit_entry(entry).await;
            }
        });
    }

    // Create a Contact, then update it — both as "sales-7".
    let c = atomo.client();
    let mut data = HashMap::new();
    data.insert("firstName".to_string(), serde_json::json!("Grace"));
    data.insert("lastName".to_string(), serde_json::json!("Hopper"));
    data.insert("email".to_string(), serde_json::json!("grace@navy.mil"));
    let contact = c
        .create("Contact", &data, &[], Some("sales-7"))
        .await
        .unwrap();
    let id = contact
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    let mut patch = HashMap::new();
    patch.insert("lastName".to_string(), serde_json::json!("Hopper-Updated"));
    c.update_many(
        "Contact",
        &[atomo::query::WhereClause {
            field: "id".into(),
            operator: atomo::query::WhereOperator::Equals,
            value: serde_json::json!(id),
        }],
        &patch,
        &[],
        Some("sales-7"),
    )
    .await
    .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    // Both a Create and an Update audit entry for Contact, attributed to sales-7.
    let rows: Vec<(i16, Option<String>)> = sqlx::query_as(
        "SELECT operation, user_id FROM audit_log WHERE entity_type = 'Contact' ORDER BY created_at",
    )
    .fetch_all(atomo.db_pool())
    .await
    .unwrap();
    assert!(
        rows.iter()
            .any(|(op, u)| *op == AuditOperation::Create as i16 && u.as_deref() == Some("sales-7")),
        "Contact create not audited: {:?}",
        rows
    );
    assert!(
        rows.iter()
            .any(|(op, u)| *op == AuditOperation::Update as i16 && u.as_deref() == Some("sales-7")),
        "Contact update not audited: {:?}",
        rows
    );

    for t in ["contact", "company", "deal", "activity"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", t))
            .execute(atomo.db_pool())
            .await
            .ok();
    }
}
