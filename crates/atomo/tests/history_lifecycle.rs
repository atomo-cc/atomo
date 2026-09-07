//! Serial, isolated PostgreSQL lifecycle coverage. DATABASE_URL is required.
use atomo::history::{HistoryConfig, HistoryMode, HistoryPolicy};
use serde_json::{json, Value};
use std::collections::HashMap;
const SCHEMA: &str = "export interface LifecycleNote { id: string; title: string; }\nexport const schema = { models: { LifecycleNote: { tableName: 'lifecycle_notes' } } }; export default schema;";
fn row(title: &str) -> HashMap<String, Value> {
    HashMap::from([("title".into(), json!(title))])
}
async fn instance(config: HistoryConfig) -> atomo::Atomo {
    atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(std::env::var("DATABASE_URL").expect("isolated DATABASE_URL"))
        .history_config(config)
        .build()
        .await
        .unwrap()
}
#[tokio::test]
async fn unknown_history_model_rejected_before_database_connection() {
    let mut config = HistoryConfig::default();
    config
        .models
        .insert("MisspelledModel".into(), HistoryPolicy::default());
    let result = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url("deliberately-invalid-database-url")
        .history_config(config)
        .build()
        .await;
    let error = match result {
        Ok(_) => panic!("unknown history model accepted"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("unknown history policy model: MisspelledModel"),
        "{error}"
    );
}
#[tokio::test]
#[ignore]
async fn replay_waits_for_retention_transaction_and_then_rejects_gap() {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let store = atomo::event_store::EventStore::new(pool.clone());
    store.init().await.unwrap();
    sqlx::query("INSERT INTO model_history_coverage(model_name,mode,complete) VALUES ('ReplayRetentionProbe','full',TRUE) ON CONFLICT(model_name) DO UPDATE SET mode='full',complete=TRUE").execute(&pool).await.unwrap();
    let mut pruning = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(718206091)")
        .execute(&mut *pruning)
        .await
        .unwrap();
    sqlx::query("UPDATE model_history_coverage SET complete=FALSE,first_gap_at=NOW() WHERE model_name='ReplayRetentionProbe'").execute(&mut *pruning).await.unwrap();
    {
        let replay = store.replay("ReplayRetentionProbe", None);
        tokio::pin!(replay);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), &mut replay)
                .await
                .is_err(),
            "replay must await lifecycle lock before checking completeness"
        );
        pruning.commit().await.unwrap();
        assert!(replay
            .await
            .unwrap_err()
            .to_string()
            .contains("HISTORY_REPLAY_UNAVAILABLE"));
    }
    sqlx::query("DELETE FROM model_history_coverage WHERE model_name='ReplayRetentionProbe'")
        .execute(&pool)
        .await
        .unwrap();
}
#[tokio::test]
#[ignore]
async fn history_modes_cover_crud_fanout_rollback_restart_and_retention() {
    let mut config = HistoryConfig::default();
    config.models.insert(
        "LifecycleNote".into(),
        HistoryPolicy {
            mode: HistoryMode::Off,
            ..Default::default()
        },
    );
    let app = instance(config).await;
    let c = app.client();
    sqlx::query("DELETE FROM event_log WHERE model_name='LifecycleNote'")
        .execute(c.db_pool())
        .await
        .unwrap();
    sqlx::query("TRUNCATE lifecycle_notes")
        .execute(c.db_pool())
        .await
        .unwrap();
    let mut events = c.event_receiver();
    c.create("LifecycleNote", &row("one"), &[], None)
        .await
        .unwrap();
    let first_marker:(String,String)=sqlx::query_as("SELECT xmin::text,first_gap_at::text FROM model_history_coverage WHERE model_name='LifecycleNote'").fetch_one(c.db_pool()).await.unwrap();
    c.create_many("LifecycleNote", &[row("two"), row("three")], None)
        .await
        .unwrap();
    c.update_many("LifecycleNote", &[], &row("updated"), &[], None)
        .await
        .unwrap();
    assert_eq!(c.delete_many("LifecycleNote", &[], None).await.unwrap(), 3);
    assert_eq!(c.restore_many("LifecycleNote", &[], None).await.unwrap(), 3);
    assert_eq!(
        c.hard_delete_many("LifecycleNote", &[], None)
            .await
            .unwrap(),
        3
    );
    for _ in 0..15 {
        tokio::time::timeout(std::time::Duration::from_secs(2), events.recv())
            .await
            .unwrap()
            .unwrap();
    }
    assert!(events.try_recv().is_err());
    let repeated_marker:(String,String)=sqlx::query_as("SELECT xmin::text,first_gap_at::text FROM model_history_coverage WHERE model_name='LifecycleNote'").fetch_one(c.db_pool()).await.unwrap();
    assert_eq!(
        first_marker, repeated_marker,
        "repeated off writes must not produce marker row versions or move the first gap"
    );
    let bad = HashMap::from([
        ("id".into(), json!("duplicate")),
        ("title".into(), json!("bad")),
    ]);
    assert!(c
        .create_many("LifecycleNote", &[bad.clone(), bad], None)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM lifecycle_notes")
            .fetch_one(c.db_pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM event_log WHERE model_name='LifecycleNote'"
        )
        .fetch_one(c.db_pool())
        .await
        .unwrap(),
        0
    );
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT complete FROM model_history_coverage WHERE model_name='LifecycleNote'"
    )
    .fetch_one(c.db_pool())
    .await
    .unwrap());
    drop(app);
    let app = instance(HistoryConfig::default()).await;
    let c = app.client();
    c.create_many("LifecycleNote", &[row("a"), row("b"), row("c")], None)
        .await
        .unwrap();
    assert!(c
        .history_store()
        .replay("LifecycleNote", None)
        .await
        .unwrap_err()
        .to_string()
        .contains("HISTORY_REPLAY_UNAVAILABLE"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM event_log WHERE model_name='LifecycleNote'"
        )
        .fetch_one(c.db_pool())
        .await
        .unwrap(),
        3
    );
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT complete FROM model_history_coverage WHERE model_name='LifecycleNote'"
    )
    .fetch_one(c.db_pool())
    .await
    .unwrap());
    assert_eq!(
        c.history_store().maintain().await.unwrap(),
        0,
        "full mode never deletes existing history"
    );
    let mut config = HistoryConfig {
        batch_size: 2,
        ..Default::default()
    };
    config.models.insert(
        "LifecycleNote".into(),
        HistoryPolicy {
            mode: HistoryMode::Retained,
            max_age_secs: None,
            max_bytes: Some(1),
        },
    );
    let retained = instance(config).await;
    assert_eq!(
        retained.client().history_store().maintain().await.unwrap(),
        2
    );
    assert_eq!(
        retained.client().history_store().maintain().await.unwrap(),
        1
    );
    let removed: i64 = sqlx::query_scalar(
        "SELECT removed_events FROM model_history_coverage WHERE model_name='LifecycleNote'",
    )
    .fetch_one(c.db_pool())
    .await
    .unwrap();
    assert_eq!(removed, 3);
    sqlx::query("DROP TABLE lifecycle_notes")
        .execute(c.db_pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM model_history_coverage WHERE model_name='LifecycleNote'")
        .execute(c.db_pool())
        .await
        .unwrap();
}
