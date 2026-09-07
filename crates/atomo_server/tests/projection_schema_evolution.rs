//! Current-state projection schema evolution is independent of event replay. Serial PG test.
use atomo::history::{HistoryConfig, HistoryMode};
use atomo::query::{WhereClause, WhereOperator};
use atomo_projectors::{CurrentStateProjection, Projection};
use serde_json::json;
use std::collections::HashMap;

async fn wait_for_lock(pool: &sqlx::PgPool, application: &str, query_fragment: &str) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND query LIKE '%' || $2 || '%')").bind(application).bind(query_fragment).fetch_one(pool).await.unwrap();
        if waiting {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{application} did not wait at {query_fragment}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[tokio::test]
#[ignore]
async fn startup_rebuild_and_mutation_share_one_deadlock_free_lock_order() {
    use std::str::FromStr;
    let url = std::env::var("DATABASE_URL").unwrap();
    let mut config = HistoryConfig::default();
    config.default.mode = HistoryMode::Off;
    let named_url = format!(
        "{url}{}application_name=projection-lock-mutation",
        if url.contains('?') { "&" } else { "?" }
    );
    let app=atomo::Atomo::builder().schema_content("export interface ProjectionLock {id:string;title:string;} export const schema={models:{ProjectionLock:{tableName:'projection_lock_base'}}};export default schema;").database_url(named_url).history_config(config).build().await.unwrap();
    let pool = app.db_pool().clone();
    app.client()
        .create("ProjectionLock", &data("keep"), &[], None)
        .await
        .unwrap();
    let projection = std::sync::Arc::new(CurrentStateProjection::new(
        "ProjectionLock",
        "projection_lock_base",
        "projection_lock_read",
        vec!["id".into(), "title".into()],
    ));
    projection.initialize_and_sync(&pool).await.unwrap();
    let init_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::postgres::PgConnectOptions::from_str(&url)
                .unwrap()
                .application_name("projection-lock-init"),
        )
        .await
        .unwrap();
    let rebuild_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::postgres::PgConnectOptions::from_str(&url)
                .unwrap()
                .application_name("projection-lock-rebuild"),
        )
        .await
        .unwrap();
    let mut writer = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock_shared(718206091)")
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("UPDATE projection_lock_base SET title='pending' WHERE id='keep'")
        .execute(&mut *writer)
        .await
        .unwrap();
    let init_projection = projection.clone();
    let initialization =
        tokio::spawn(async move { init_projection.initialize_and_sync(&init_pool).await });
    wait_for_lock(&pool, "projection-lock-init", "IN SHARE MODE").await;
    let rebuild_projection = projection.clone();
    let rebuild = tokio::spawn(async move { rebuild_projection.rebuild(&rebuild_pool).await });
    wait_for_lock(
        &pool,
        "projection-lock-rebuild",
        "pg_advisory_xact_lock(718206091)",
    )
    .await;
    let mut notifications = app.event_receiver();
    let updating = app.clone();
    let update = tokio::spawn(async move {
        updating
            .client()
            .update_many(
                "ProjectionLock",
                &id("keep"),
                &HashMap::from([("title".into(), json!("newest"))]),
                &[],
                None,
            )
            .await
    });
    // The queued mutation must wait before taking any base locks, otherwise it can block
    // startup while waiting behind the queued exclusive rebuild: a three-way lock cycle.
    wait_for_lock(
        &pool,
        "projection-lock-mutation",
        "pg_advisory_xact_lock_shared(718206091)",
    )
    .await;
    writer.commit().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), initialization)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let error = tokio::time::timeout(std::time::Duration::from_secs(5), rebuild)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("HISTORY_REPLAY_UNAVAILABLE"));
    tokio::time::timeout(std::time::Duration::from_secs(5), update)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let event = notifications.recv().await.unwrap();
    projection
        .handle_event("Updated", &event.data, &pool)
        .await
        .unwrap();
    projection
        .handle_event("Updated", &data("keep"), &pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM projection_lock_read WHERE id='keep'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "newest"
    );
    for table in ["projection_lock_read", "projection_lock_base"] {
        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM model_history_coverage WHERE model_name='ProjectionLock'")
        .execute(&pool)
        .await
        .unwrap();
}
const OLD:&str="export interface ProjectionEvolution {id:string;title:string;} export const schema={models:{ProjectionEvolution:{tableName:'projection_evolution_base'}}};export default schema;";
const NEW:&str="export interface ProjectionEvolution {id:string;title:string;sourceSequence?:number;} export const schema={models:{ProjectionEvolution:{tableName:'projection_evolution_base'}}};export default schema;";
async fn app(schema: &str) -> atomo::Atomo {
    let mut history = HistoryConfig::default();
    history.default.mode = HistoryMode::Off;
    atomo::Atomo::builder()
        .schema_content(schema)
        .database_url(std::env::var("DATABASE_URL").unwrap())
        .history_config(history)
        .build()
        .await
        .unwrap()
}
fn id(value: &str) -> Vec<WhereClause> {
    vec![WhereClause {
        field: "id".into(),
        operator: WhereOperator::Equals,
        value: json!(value),
    }]
}
fn data(value: &str) -> HashMap<String, serde_json::Value> {
    HashMap::from([("id".into(), json!(value)), ("title".into(), json!("old"))])
}
#[tokio::test]
#[ignore]
async fn history_off_schema_restart_adds_columns_and_repairs_current_state() {
    let old = app(OLD).await;
    let pool = old.db_pool().clone();
    old.client()
        .create_many("ProjectionEvolution", &[data("keep"), data("missed")], None)
        .await
        .unwrap();
    let projection = CurrentStateProjection::new(
        "ProjectionEvolution",
        "projection_evolution_base",
        "projection_evolution_read",
        vec!["id".into(), "title".into()],
    );
    projection.initialize_and_sync(&pool).await.unwrap();
    sqlx::query("ALTER TABLE projection_evolution_read ADD COLUMN legacy TEXT")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE projection_evolution_read SET legacy='preserve' WHERE id='keep'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM projection_evolution_read WHERE id='missed'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO projection_evolution_read(id,title) VALUES('orphan','stale')")
        .execute(&pool)
        .await
        .unwrap();
    drop(old);
    let restarted = app(NEW).await;
    restarted
        .client()
        .update_many(
            "ProjectionEvolution",
            &id("keep"),
            &HashMap::from([
                ("title".into(), json!("current")),
                ("sourceSequence".into(), json!(7)),
            ]),
            &[],
            None,
        )
        .await
        .unwrap();
    let projection = CurrentStateProjection::new(
        "ProjectionEvolution",
        "projection_evolution_base",
        "projection_evolution_read",
        vec!["id".into(), "title".into(), "source_sequence".into()],
    );
    assert_eq!(projection.initialize_and_sync(&pool).await.unwrap(), 2);
    let before_xmin: String =
        sqlx::query_scalar("SELECT xmin::text FROM projection_evolution_read WHERE id='keep'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        projection.initialize_and_sync(&pool).await.unwrap(),
        0,
        "unchanged startup performs no row rewrites"
    );
    let after_xmin: String =
        sqlx::query_scalar("SELECT xmin::text FROM projection_evolution_read WHERE id='keep'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before_xmin, after_xmin);
    let existing: (String, String, String) = sqlx::query_as(
        "SELECT title,source_sequence,legacy FROM projection_evolution_read WHERE id='keep'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(existing, ("current".into(), "7".into(), "preserve".into()));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projection_evolution_read WHERE id='missed'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projection_evolution_read WHERE id='orphan'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    projection
        .handle_event("Updated", &data("keep"), &pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT title FROM projection_evolution_read WHERE id='keep'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "current",
        "queued old payload cannot overwrite current state"
    );
    let mut events = restarted.event_receiver();
    restarted
        .client()
        .create("ProjectionEvolution", &data("fresh"), &[], None)
        .await
        .unwrap();
    let event = events.recv().await.unwrap();
    projection
        .handle_event("Created", &event.data, &pool)
        .await
        .unwrap();
    restarted
        .client()
        .update_many(
            "ProjectionEvolution",
            &id("fresh"),
            &HashMap::from([("sourceSequence".into(), json!(9))]),
            &[],
            None,
        )
        .await
        .unwrap();
    let event = events.recv().await.unwrap();
    projection
        .handle_event("Updated", &event.data, &pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT source_sequence FROM projection_evolution_read WHERE id='fresh'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "9"
    );
    restarted
        .client()
        .hard_delete_many("ProjectionEvolution", &id("fresh"), None)
        .await
        .unwrap();
    let event = events.recv().await.unwrap();
    projection
        .handle_event("HardDeleted", &event.data, &pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projection_evolution_read")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert!(projection
        .rebuild(&pool)
        .await
        .unwrap_err()
        .to_string()
        .contains("HISTORY_REPLAY_UNAVAILABLE"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM event_log WHERE model_name='ProjectionEvolution'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    sqlx::query("CREATE TABLE projection_evolution_bad(id TEXT PRIMARY KEY,title INTEGER)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO projection_evolution_bad VALUES('sentinel',12)")
        .execute(&pool)
        .await
        .unwrap();
    let incompatible = CurrentStateProjection::new(
        "ProjectionEvolution",
        "projection_evolution_base",
        "projection_evolution_bad",
        vec!["id".into(), "title".into(), "source_sequence".into()],
    );
    assert!(incompatible
        .initialize_and_sync(&pool)
        .await
        .unwrap_err()
        .to_string()
        .contains("CURRENT_PROJECTION_SCHEMA_CONFLICT"));
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT title FROM projection_evolution_bad WHERE id='sentinel'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        12
    );
    for table in [
        "projection_evolution_bad",
        "projection_evolution_read",
        "projection_evolution_base",
    ] {
        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM model_history_coverage WHERE model_name='ProjectionEvolution'")
        .execute(&pool)
        .await
        .unwrap();
}
