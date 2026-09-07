//! Real HTTP/auth/CRUD/audit wiring against the serial isolated PostgreSQL suite.
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn request(
    app: &axum::Router,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request =
        Request::builder()
            .uri(path)
            .method(if body.is_some() { "POST" } else { "GET" });
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let payload = if let Some(body) = body {
        request = request.header("content-type", "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(request.body(payload).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
#[ignore]
async fn authenticated_diagnostics_and_history_off_http_mutations() {
    let schema = r#"export interface StorageProbe { id: string; title: string; }
        export const schema={models:{StorageProbe:{tableName:"storage_http_probes"}}}; export default schema;"#;
    let mut history = atomo::history::HistoryConfig::default();
    history.models.insert(
        "StorageProbe".into(),
        atomo::history::HistoryPolicy {
            mode: atomo::history::HistoryMode::Off,
            ..Default::default()
        },
    );
    let atomo = atomo::Atomo::builder()
        .schema_content(schema)
        .database_url(std::env::var("DATABASE_URL").unwrap())
        .history_config(history)
        .build()
        .await
        .unwrap();
    let pool = atomo.db_pool();
    atomo_server::ensure_platform_tables(pool).await.unwrap();
    let auth = atomo_server::auth::HttpAuthService::new("storage-http-test-secret", pool.clone());
    let mut audit_config = atomo::history::AuditConfig::default();
    audit_config.models.insert(
        "StorageProbe".into(),
        atomo::history::AuditPolicy {
            mode: atomo::history::AuditMode::Metadata,
            ..Default::default()
        },
    );
    let audit =
        atomo_server::audit::HttpAuditService::with_config(pool.clone(), audit_config).unwrap();
    let _listener =
        atomo_server::audit::spawn_model_audit_listener(audit.clone(), atomo.event_receiver());
    let redirects = Arc::new(atomo_server::public_read_redirects::RedirectStore::new(
        pool.clone(),
    ));
    redirects.init().await.unwrap();
    let app = atomo_server::handlers::create_router(
        atomo_server::handlers::build_extended_schema(&atomo),
        atomo.clone(),
        auth.clone(),
        audit,
        atomo_server::auth::RegistrationConfig::new(false),
        vec![],
        redirects,
    );
    assert_eq!(
        request(&app, "/storage/diagnostics", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let password = "StorageHttpTest9";
    let password_hash = auth.hash_password(password).unwrap();
    let mut users = Vec::new();
    let mut admin_token = String::new();
    for role in ["viewer", "admin"] {
        let id = atomo_core::types::EntityId::new().to_string();
        let email = format!("{id}@storage-test.invalid");
        sqlx::query("INSERT INTO users(id,email,password_hash,role,first_name,last_name,is_active) VALUES($1,$2,$3,$4,'Storage','Test',TRUE)")
            .bind(&id).bind(&email).bind(&password_hash).bind(role).execute(pool).await.unwrap();
        users.push(id);
        let (status, login) = request(
            &app,
            "/auth/login",
            None,
            Some(json!({"email":email,"password":password})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{login}");
        let token = login["token"].as_str().unwrap();
        let (status, diagnostics) = request(&app, "/storage/diagnostics", Some(token), None).await;
        if role == "viewer" {
            assert_eq!(status, StatusCode::FORBIDDEN);
        } else {
            assert_eq!(status, StatusCode::OK, "{diagnostics}");
            assert!(diagnostics.get("cache").is_some());
            assert!(diagnostics.get("history").is_some());
            assert!(diagnostics.get("audit").is_some());
            admin_token = token.to_string();
        }
    }
    sqlx::query("DELETE FROM event_log WHERE model_name='StorageProbe'")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM audit_log WHERE entity_type='StorageProbe'")
        .execute(pool)
        .await
        .unwrap();
    let id = atomo_core::types::EntityId::new().to_string();
    let (status, result) = request(
        &app,
        "/graphql",
        Some(&admin_token),
        Some(json!({
            "query":"mutation($data:JSON!){create(model:\"StorageProbe\",data:$data)}",
            "variables":{"data":{"id":id,"title":"payload must not enter metadata audit"}}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(result.get("errors").is_none(), "{result}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM event_log WHERE model_name='StorageProbe'"
        )
        .fetch_one(pool)
        .await
        .unwrap(),
        0
    );
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let details: Option<String> = sqlx::query_scalar("SELECT operation_details::text FROM audit_log WHERE entity_type='StorageProbe' AND entity_id::text=$1")
            .bind(&id).fetch_optional(pool).await.unwrap();
        if let Some(details) = details {
            assert_eq!(details, "{}");
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "post-commit audit listener did not receive HTTP mutation"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    drop(_listener);
    sqlx::query("DROP TABLE storage_http_probes")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM audit_log WHERE entity_type='StorageProbe'")
        .execute(pool)
        .await
        .unwrap();
    for id in users {
        sqlx::query("DELETE FROM sessions WHERE user_id::text=$1")
            .bind(&id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id::text=$1")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
    }
}
