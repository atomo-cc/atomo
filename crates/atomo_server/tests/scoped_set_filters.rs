//! Real PostgreSQL set filters across CRUD and live projection events. Run serially.
use atomo::query::{WhereClause, WhereOperator};
use serde_json::{json, Value};
use std::collections::HashMap;
const SCHEMA:&str="export interface SetFilterProbe { id: string; worldId: string; sequence: number; payload: any; }\nexport const schema={models:{SetFilterProbe:{tableName:'set_filter_probes'}}}; export default schema;";
fn clause(field: &str, operator: WhereOperator, value: Value) -> WhereClause {
    WhereClause {
        field: field.into(),
        operator,
        value,
    }
}
async fn project(
    manager: &atomo_projectors::ProjectorManager,
    rx: &mut tokio::sync::broadcast::Receiver<atomo::events::ModelEvent>,
    count: usize,
) {
    for _ in 0..count {
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        manager
            .process_event(
                &format!("{:?}", event.event_type),
                &event.model_name,
                &event.data,
            )
            .await
            .unwrap();
    }
}
#[tokio::test]
#[ignore]
async fn scoped_set_filters_preserve_other_world_and_project_deletions() {
    let app = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(std::env::var("DATABASE_URL").unwrap())
        .build()
        .await
        .unwrap();
    let client = app.client();
    let pool = app.db_pool();
    sqlx::query("TRUNCATE set_filter_probes")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE IF NOT EXISTS set_filter_probes_projection(id TEXT PRIMARY KEY,world_id TEXT,sequence TEXT,payload TEXT)").execute(pool).await.unwrap();
    sqlx::query("TRUNCATE set_filter_probes_projection")
        .execute(pool)
        .await
        .unwrap();
    let mut manager = atomo_projectors::ProjectorManager::new(pool.clone());
    manager.register(atomo_projectors::TableProjection::new(
        "SetFilterProbe",
        "set_filter_probes_projection",
        vec![
            "id".into(),
            "world_id".into(),
            "sequence".into(),
            "payload".into(),
        ],
    ));
    let mut rx = client.event_receiver();
    let rows: Vec<HashMap<String, Value>> = [("a1", "a", 1), ("a2", "a", 2), ("b1", "b", 1)]
        .into_iter()
        .map(|(id, world, sequence)| {
            HashMap::from([
                ("id".into(), json!(id)),
                ("worldId".into(), json!(world)),
                ("sequence".into(), json!(sequence)),
                ("payload".into(), json!(["json",1,{"nested":true}])),
            ])
        })
        .collect();
    client
        .create_many("SetFilterProbe", &rows, None)
        .await
        .unwrap();
    project(&manager, &mut rx, 3).await;
    let payload: Value = sqlx::query_scalar("SELECT payload FROM set_filter_probes WHERE id='b1'")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(payload, json!(["json",1,{"nested":true}]));
    let scope = vec![
        clause("worldId", WhereOperator::Equals, json!("a")),
        clause("sequence", WhereOperator::In, json!([1, 2])),
    ];
    assert_eq!(client.count("SetFilterProbe", &scope).await.unwrap(), 2);
    let exclusion = vec![
        clause("worldId", WhereOperator::Equals, json!("a")),
        clause("id", WhereOperator::NotIn, json!(["a1"])),
    ];
    assert_eq!(client.count("SetFilterProbe", &exclusion).await.unwrap(), 1);
    assert_eq!(
        client
            .count(
                "SetFilterProbe",
                &[clause("id", WhereOperator::NotIn, json!([]))]
            )
            .await
            .unwrap(),
        3
    );
    let update = HashMap::from([("payload".into(), json!(["updated", 2]))]);
    assert_eq!(
        client
            .update_many("SetFilterProbe", &scope, &update, &[], None)
            .await
            .unwrap()
            .len(),
        2
    );
    project(&manager, &mut rx, 2).await;
    let ids = vec![
        clause("worldId", WhereOperator::Equals, json!("a")),
        clause("id", WhereOperator::In, json!(["a1", "b1"])),
    ];
    assert_eq!(
        client
            .hard_delete_many("SetFilterProbe", &ids, None)
            .await
            .unwrap(),
        1
    );
    project(&manager, &mut rx, 1).await;
    for table in ["set_filter_probes", "set_filter_probes_projection"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table} WHERE id='b1'"))
                .fetch_one(pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table} WHERE id='a1'"))
                .fetch_one(pool)
                .await
                .unwrap(),
            0
        );
    }
    let empty = vec![clause("id", WhereOperator::In, json!([]))];
    assert_eq!(
        client
            .hard_delete_many("SetFilterProbe", &empty, None)
            .await
            .unwrap(),
        0
    );
    let invalid = vec![clause("id", WhereOperator::NotIn, json!("invalid"))];
    assert!(client
        .hard_delete_many("SetFilterProbe", &invalid, None)
        .await
        .unwrap_err()
        .to_string()
        .contains("INVALID_FILTER"));
    assert_eq!(
        client
            .delete_many("SetFilterProbe", &scope, None)
            .await
            .unwrap(),
        1
    );
    project(&manager, &mut rx, 1).await;
    assert_eq!(
        client
            .restore_many("SetFilterProbe", &scope, None)
            .await
            .unwrap(),
        1
    );
    project(&manager, &mut rx, 1).await;
    assert_eq!(
        client
            .hard_delete_many("SetFilterProbe", &scope, None)
            .await
            .unwrap(),
        1
    );
    project(&manager, &mut rx, 1).await;
    for table in ["set_filter_probes", "set_filter_probes_projection"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(pool)
                .await
                .unwrap(),
            1
        );
        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM event_log WHERE model_name='SetFilterProbe'")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM model_history_coverage WHERE model_name='SetFilterProbe'")
        .execute(pool)
        .await
        .unwrap();
}
