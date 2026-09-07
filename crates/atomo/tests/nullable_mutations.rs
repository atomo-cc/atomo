//! Explicit nulls must retain database column types across every mutation shape.
use atomo::query::{WhereClause, WhereOperator};
use serde_json::{json, Value};
use std::collections::HashMap;
const SCHEMA:&str="export interface NullableMutation {id:string;requiredLabel:string;sequence?:number;enabled?:boolean;occurredAt?:Date;metadata?:any;} export const schema={models:{NullableMutation:{tableName:'nullable_mutation_probes'}}};export default schema;";
fn record(id: &str, sequence: Value) -> HashMap<String, Value> {
    HashMap::from([
        ("id".into(), json!(id)),
        ("requiredLabel".into(), json!("retained")),
        ("sequence".into(), sequence),
        ("enabled".into(), Value::Null),
        ("occurredAt".into(), Value::Null),
        ("metadata".into(), Value::Null),
    ])
}
#[tokio::test]
#[ignore]
async fn nullable_fields_work_in_single_bulk_and_scoped_updates() {
    let url = std::env::var("DATABASE_URL").unwrap();
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    // This fixture owns exactly one base table; remove only an interrupted run of itself.
    sqlx::query("DROP TABLE IF EXISTS nullable_mutation_probes")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM event_log WHERE model_name='NullableMutation'")
        .execute(&pool)
        .await
        .unwrap();
    let app = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(url)
        .build()
        .await
        .unwrap();
    let client = app.client();
    client
        .create(
            "NullableMutation",
            &record("single", Value::Null),
            &[],
            None,
        )
        .await
        .unwrap();
    let mut populated = record("populated", json!(7));
    populated.insert("enabled".into(), json!(false));
    populated.insert("occurredAt".into(), json!("2026-01-01T00:00:00Z"));
    populated.insert("metadata".into(), json!([1,{"nested":null}]));
    client
        .create_many(
            "NullableMutation",
            &[
                record("empty", Value::Null),
                populated,
                record("zero", json!(0)),
            ],
            None,
        )
        .await
        .unwrap();
    let target = vec![WhereClause {
        field: "id".into(),
        operator: WhereOperator::In,
        value: json!(["populated", "zero"]),
    }];
    let cleared = HashMap::from([
        ("sequence".into(), Value::Null),
        ("enabled".into(), Value::Null),
        ("occurredAt".into(), Value::Null),
        ("metadata".into(), Value::Null),
        ("requiredLabel".into(), json!("cleared")),
    ]);
    assert_eq!(
        client
            .update_many("NullableMutation", &target, &cleared, &[], None)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM nullable_mutation_probes WHERE sequence IS NULL AND enabled IS NULL AND occurred_at IS NULL AND metadata IS NULL").fetch_one(&pool).await.unwrap(),4);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT required_label FROM nullable_mutation_probes WHERE id='single'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "retained"
    );
    assert!(
        client
            .update_many(
                "NullableMutation",
                &target,
                &HashMap::from([("requiredLabel".into(), Value::Null)]),
                &[],
                None
            )
            .await
            .is_err(),
        "explicit null must not weaken NOT NULL constraints"
    );
    let mut omitted = record("omitted", Value::Null);
    omitted.remove("occurredAt");
    client
        .create_many(
            "NullableMutation",
            &[record("fallback", Value::Null), omitted],
            None,
        )
        .await
        .unwrap();
    let mut invalid = record("invalid", Value::Null);
    invalid.insert("requiredLabel".into(), Value::Null);
    assert!(client
        .create_many(
            "NullableMutation",
            &[record("rollback", json!(3)), invalid],
            None
        )
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM nullable_mutation_probes")
            .fetch_one(&pool)
            .await
            .unwrap(),
        6,
        "failed nullable batch must roll back every row"
    );
    sqlx::query("DROP TABLE nullable_mutation_probes")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM event_log WHERE model_name='NullableMutation'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM model_history_coverage WHERE model_name='NullableMutation'")
        .execute(&pool)
        .await
        .unwrap();
}
