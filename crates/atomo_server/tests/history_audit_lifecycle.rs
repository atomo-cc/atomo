//! Opt-in PostgreSQL tests; serial execution against an isolated database.
use atomo::history::{AuditConfig, AuditMode, AuditPolicy};
use atomo_core::{
    audit::{AuditLogEntry, AuditOperation, AuditService},
    types::EntityId,
};
use atomo_projectors::{Projection, ProjectorManager, TableProjection};
#[tokio::test]
#[ignore]
async fn audit_modes_and_retention_redact_and_bound_without_touching_auth() {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    atomo_server::ensure_platform_tables(&pool).await.unwrap();
    sqlx::query("DELETE FROM audit_log WHERE entity_type='LifecycleAudit'")
        .execute(&pool)
        .await
        .unwrap();
    let mut config = AuditConfig::default();
    config.models.insert(
        "LifecycleAudit".into(),
        AuditPolicy {
            mode: AuditMode::Metadata,
            ..Default::default()
        },
    );
    let audit =
        atomo_server::audit::HttpAuditService::with_config(pool.clone(), config.clone()).unwrap();
    let entry = || {
        let mut e = AuditLogEntry::new(
            "LifecycleAudit",
            EntityId::new(),
            AuditOperation::Create,
            r#"{"private":"never retain"}"#,
            None,
        );
        e.ip_address = Some("192.0.2.1".into());
        e.user_agent = Some("private agent".into());
        e
    };
    audit.log_audit_entry(entry()).await.unwrap();
    let (details, ip, agent): (String, Option<String>, Option<String>) = sqlx::query_as("SELECT operation_details::text,ip_address,user_agent FROM audit_log WHERE entity_type='LifecycleAudit'").fetch_one(&pool).await.unwrap();
    assert_eq!(details, "{}");
    assert!(ip.is_none());
    assert!(agent.is_none());
    config.models.get_mut("LifecycleAudit").unwrap().mode = AuditMode::Off;
    atomo_server::audit::HttpAuditService::with_config(pool.clone(), config.clone())
        .unwrap()
        .log_audit_entry(entry())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_log WHERE entity_type='LifecycleAudit'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    config.models.get_mut("LifecycleAudit").unwrap().mode = AuditMode::Full;
    let full =
        atomo_server::audit::HttpAuditService::with_config(pool.clone(), config.clone()).unwrap();
    full.log_audit_entry(entry()).await.unwrap();
    assert_eq!(full.maintain().await.unwrap(), 0);
    config
        .models
        .get_mut("LifecycleAudit")
        .unwrap()
        .max_age_secs = Some(i64::MAX as u64);
    assert_eq!(
        atomo_server::audit::HttpAuditService::with_config(pool.clone(), config.clone())
            .unwrap()
            .maintain()
            .await
            .unwrap(),
        0,
        "accepted large ages must not overflow PostgreSQL timestamps"
    );
    config.batch_size = 1;
    config
        .models
        .get_mut("LifecycleAudit")
        .unwrap()
        .max_age_secs = Some(1);
    sqlx::query("UPDATE audit_log SET created_at=NOW()-INTERVAL '1 hour' WHERE entity_type='LifecycleAudit'").execute(&pool).await.unwrap();
    let retained =
        atomo_server::audit::HttpAuditService::with_config(pool.clone(), config).unwrap();
    assert_eq!(retained.maintain().await.unwrap(), 1);
    assert_eq!(retained.maintain().await.unwrap(), 1);
}

#[tokio::test]
#[ignore]
async fn all_projection_histories_are_preflighted_before_any_clear() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let store = atomo::event_store::EventStore::new(pool.clone());
    store.init().await.unwrap();
    for table in ["lifecycle_projection_a", "lifecycle_projection_b"] {
        sqlx::query(&format!(
            "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY)"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(&format!(
            "INSERT INTO {table} VALUES ('keep') ON CONFLICT DO NOTHING"
        ))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query("INSERT INTO model_history_coverage(model_name,mode,complete) VALUES ('LifecycleProjectionA','full',TRUE),('LifecycleProjectionB','off',FALSE) ON CONFLICT(model_name) DO UPDATE SET mode=EXCLUDED.mode,complete=EXCLUDED.complete").execute(&pool).await.unwrap();
    let mut manager = ProjectorManager::new(pool.clone());
    manager.register(TableProjection::new(
        "LifecycleProjectionA",
        "lifecycle_projection_a",
        vec!["id".into()],
    ));
    manager.register(TableProjection::new(
        "LifecycleProjectionB",
        "lifecycle_projection_b",
        vec!["id".into()],
    ));
    assert!(manager
        .rebuild_all()
        .await
        .unwrap_err()
        .to_string()
        .contains("HISTORY_REPLAY_UNAVAILABLE"));
    let single = TableProjection::new(
        "LifecycleProjectionB",
        "lifecycle_projection_b",
        vec!["id".into()],
    );
    assert!(single.rebuild(&pool).await.is_err());
    // A concurrent history-off transaction cannot race preflight and erase a projection.
    sqlx::query("UPDATE model_history_coverage SET mode='full',complete=TRUE WHERE model_name='LifecycleProjectionB'").execute(&pool).await.unwrap();
    let mut off_config = atomo::history::HistoryConfig::default();
    off_config.default.mode = atomo::history::HistoryMode::Off;
    let off = atomo::event_store::EventStore::with_config(pool.clone(), off_config).unwrap();
    let event = atomo::events::ModelEvent {
        event_type: atomo::events::EventType::Created,
        model_name: "LifecycleProjectionB".into(),
        data: Default::default(),
        previous_data: None,
        timestamp: "2026-01-01T00:00:00Z".into(),
        event_id: "unrecorded".into(),
        actor: None,
        origin: None,
    };
    let writing_pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut write = writing_pool.begin().await.unwrap();
    off.persist_in(&mut *write, &event).await.unwrap();
    {
        let rebuilding = manager.rebuild_all();
        tokio::pin!(rebuilding);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), &mut rebuilding)
                .await
                .is_err()
        );
        write.commit().await.unwrap();
        assert!(rebuilding
            .await
            .unwrap_err()
            .to_string()
            .contains("HISTORY_REPLAY_UNAVAILABLE"));
    }
    use tower::ServiceExt;
    let manager = std::sync::Arc::new(manager);
    atomo_server::ensure_platform_tables(&pool).await.unwrap();
    let auth = atomo_server::auth::HttpAuthService::new("projection-test-secret", pool.clone());
    let router = atomo_server::projector_routes::authenticated_projector_router(
        manager.clone(),
        auth.clone(),
    );
    let legacy = atomo_server::projector_routes::projector_router(manager.clone());
    for app in [&legacy, &router] {
        for (method, path) in [("GET", "/projections"), ("POST", "/projections/rebuild")] {
            let response = app
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method(method)
                        .uri(path)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        }
    }
    for role in ["viewer", "admin"] {
        let id = EntityId::new().to_string();
        let email = format!("{id}@projection-test.invalid");
        sqlx::query("INSERT INTO users(id,email,password_hash,first_name,last_name,role,is_active) VALUES($1,$2,'unused','Projection','Test',$3,TRUE)").bind(&id).bind(&email).bind(role).execute(&pool).await.unwrap();
        let (token, _) = auth.issue_tokens(&id, &email, role).await.unwrap();
        for (method, path) in [("GET", "/projections"), ("POST", "/projections/rebuild")] {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method(method)
                        .uri(path)
                        .header("authorization", format!("Bearer {token}"))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            if role == "viewer" {
                assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
            } else if method == "GET" {
                assert_eq!(response.status(), axum::http::StatusCode::OK);
            } else {
                assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
                let response: serde_json::Value = serde_json::from_slice(
                    &axum::body::to_bytes(response.into_body(), 4096)
                        .await
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(response["code"], "HISTORY_REPLAY_UNAVAILABLE");
                assert!(!response.to_string().contains("LifecycleProjection"));
            }
        }
        sqlx::query("DELETE FROM sessions WHERE user_id=$1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
    }
    for table in ["lifecycle_projection_a", "lifecycle_projection_b"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }
    sqlx::query("UPDATE model_history_coverage SET mode='full',complete=TRUE WHERE model_name IN ('LifecycleProjectionA','LifecycleProjectionB')").execute(&pool).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), manager.rebuild_all())
        .await
        .expect("single-connection rebuild must not deadlock")
        .unwrap();
    for table in ["lifecycle_projection_a", "lifecycle_projection_b"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM model_history_coverage WHERE model_name IN ('LifecycleProjectionA','LifecycleProjectionB')").execute(&pool).await.unwrap();
}
