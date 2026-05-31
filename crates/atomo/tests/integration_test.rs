//! Integration tests for CRUD → Event Store → Subscription pipeline
//!
//! These tests require a running PostgreSQL instance.
//! Set DATABASE_URL env var to run: DATABASE_URL=postgresql://localhost/atomo_test cargo test -p atomo --test integration_test

use std::collections::HashMap;
use serde_json::{json, Value};

/// Helper to create a test schema
fn test_schema() -> atomo_schema::Schema {
    let mut fields = HashMap::new();
    fields.insert("id".to_string(), atomo_schema::Field {
        name: "id".to_string(),
        field_type: atomo_schema::FieldType::EntityId,
        optional: false,
        attributes: vec![atomo_schema::FieldAttribute::Primary],
    });
    fields.insert("name".to_string(), atomo_schema::Field {
        name: "name".to_string(),
        field_type: atomo_schema::FieldType::String,
        optional: false,
        attributes: vec![],
    });
    fields.insert("email".to_string(), atomo_schema::Field {
        name: "email".to_string(),
        field_type: atomo_schema::FieldType::String,
        optional: true,
        attributes: vec![],
    });
    let mut models = HashMap::new();
    models.insert("TestUser".to_string(), atomo_schema::Model {
        name: "TestUser".to_string(),
        fields,
        access: None,
        hooks: None,
        validation: std::collections::HashMap::new(),
    });
    atomo_schema::Schema { models }
}

#[tokio::test]
#[ignore] // Requires DATABASE_URL
async fn test_create_emits_event_and_persists() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()))
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    // Subscribe before creating
    let mut rx = client.subscribe("TestUser", &[], &[]).await;

    // Create a record
    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("Alice"));
    data.insert("email".to_string(), json!("alice@example.com"));
    let record = client.create("TestUser", &data, &[]).await.expect("create failed");
    assert!(!record.is_empty());

    // Verify event was received via subscription
    let event = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        rx.recv()
    ).await.expect("timeout").expect("recv failed");
    assert_eq!(event.model_name, "TestUser");
    assert!(matches!(event.event_type, atomo::events::EventType::Created));
}

#[tokio::test]
#[ignore]
async fn test_find_many_returns_created_records() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()))
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("Bob"));
    client.create("TestUser", &data, &[]).await.expect("create failed");

    let results = client.find_many("TestUser", &[], &[], None, None, &[]).await.expect("find failed");
    assert!(!results.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_soft_delete_hides_records() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()))
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("ToDelete"));
    let record = client.create("TestUser", &data, &[]).await.expect("create failed");

    // Delete (soft)
    use atomo::query::{WhereClause, WhereOperator};
    let id = record.get("id").cloned().unwrap_or(Value::Null);
    let where_clauses = vec![WhereClause { field: "id".to_string(), operator: WhereOperator::Equals, value: id }];
    let count = client.delete_many("TestUser", &where_clauses).await.expect("delete failed");
    assert_eq!(count, 1);

    // Should not appear in find_many
    let results = client.find_many("TestUser", &where_clauses, &[], None, None, &[]).await.expect("find failed");
    assert!(results.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_event_store_replay() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()))
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("EventTest"));
    client.create("TestUser", &data, &[]).await.expect("create failed");

    // Replay events
    let store = atomo::event_store::EventStore::new(client.db_pool().clone());
    let events = store.replay("TestUser", None).await.expect("replay failed");
    assert!(!events.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_update_many_modifies_and_emits() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()))
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("Before"));
    let record = client.create("TestUser", &data, &[]).await.expect("create failed");
    let id = record.get("id").cloned().unwrap();

    let mut rx = client.subscribe("TestUser", &[], &[]).await;

    use atomo::query::{WhereClause, WhereOperator};
    let where_clauses = vec![WhereClause { field: "id".to_string(), operator: WhereOperator::Equals, value: id }];
    let mut update_data = HashMap::new();
    update_data.insert("name".to_string(), json!("After"));
    let updated = client.update_many("TestUser", &where_clauses, &update_data, &[]).await.expect("update failed");
    assert_eq!(updated[0].get("name").unwrap(), &json!("After"));

    let event = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        rx.recv()
    ).await.expect("timeout").expect("recv failed");
    assert!(matches!(event.event_type, atomo::events::EventType::Updated));
}

#[tokio::test]
#[ignore]
async fn test_delete_emits_deleted_event() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()))
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("DeleteMe"));
    let record = client.create("TestUser", &data, &[]).await.expect("create failed");
    let id = record.get("id").cloned().unwrap();

    let mut rx = client.subscribe("TestUser", &[], &[]).await;

    use atomo::query::{WhereClause, WhereOperator};
    let where_clauses = vec![WhereClause { field: "id".to_string(), operator: WhereOperator::Equals, value: id }];
    let count = client.delete_many("TestUser", &where_clauses).await.expect("delete failed");
    assert_eq!(count, 1);

    let event = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        rx.recv()
    ).await.expect("timeout").expect("recv failed");
    assert!(matches!(event.event_type, atomo::events::EventType::Deleted));
}

#[tokio::test]
#[ignore]
async fn test_count_excludes_soft_deleted() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()))
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data1 = HashMap::new();
    data1.insert("name".to_string(), json!("CountA"));
    let rec1 = client.create("TestUser", &data1, &[]).await.expect("create failed");

    let mut data2 = HashMap::new();
    data2.insert("name".to_string(), json!("CountB"));
    client.create("TestUser", &data2, &[]).await.expect("create failed");

    let before = client.count("TestUser", &[]).await.expect("count failed");
    assert!(before >= 2);

    use atomo::query::{WhereClause, WhereOperator};
    let id = rec1.get("id").cloned().unwrap();
    let where_clauses = vec![WhereClause { field: "id".to_string(), operator: WhereOperator::Equals, value: id }];
    client.delete_many("TestUser", &where_clauses).await.expect("delete failed");

    let after = client.count("TestUser", &[]).await.expect("count failed");
    assert_eq!(after, before - 1);
}

#[tokio::test]
#[ignore]
async fn test_find_unique_by_id() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()))
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("UniqueFind"));
    let record = client.create("TestUser", &data, &[]).await.expect("create failed");
    let id = record.get("id").cloned().unwrap();

    use atomo::query::{WhereClause, WhereOperator};
    let where_clauses = vec![WhereClause { field: "id".to_string(), operator: WhereOperator::Equals, value: id }];
    let found = client.find_unique("TestUser", &where_clauses, &[]).await.expect("find_unique failed");
    assert!(found.is_some());
    assert_eq!(found.unwrap().get("name").unwrap(), &json!("UniqueFind"));
}
