//! Integration tests for CRUD → Event Store → Subscription pipeline
//!
//! These tests require a running PostgreSQL instance.
//! Set DATABASE_URL env var to run: DATABASE_URL=postgresql://localhost/atomo_test cargo test -p atomo --test integration_test

use serde_json::{json, Value};
use std::collections::HashMap;

/// Helper to create a test schema
fn test_schema() -> atomo_schema::Schema {
    let mut fields = HashMap::new();
    fields.insert(
        "id".to_string(),
        atomo_schema::Field {
            name: "id".to_string(),
            field_type: atomo_schema::FieldType::EntityId,
            optional: false,
            attributes: vec![atomo_schema::FieldAttribute::Primary],
        },
    );
    fields.insert(
        "name".to_string(),
        atomo_schema::Field {
            name: "name".to_string(),
            field_type: atomo_schema::FieldType::String,
            optional: false,
            attributes: vec![],
        },
    );
    fields.insert(
        "email".to_string(),
        atomo_schema::Field {
            name: "email".to_string(),
            field_type: atomo_schema::FieldType::String,
            optional: true,
            attributes: vec![],
        },
    );
    let mut models = HashMap::new();
    models.insert(
        "TestUser".to_string(),
        atomo_schema::Model {
            name: "TestUser".to_string(),
            fields,
            access: None,
            hooks: None,
            validation: std::collections::HashMap::new(),
            table_name: None,
            relationships: std::collections::HashMap::new(),
            constraints: Vec::new(),
            events: Default::default(),
            ui: None,
        },
    );
    atomo_schema::Schema { models, actions: std::collections::HashMap::new() }
}

#[tokio::test]
#[ignore] // Requires DATABASE_URL
async fn test_create_emits_event_and_persists() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
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
    let record = client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");
    assert!(!record.is_empty());

    // Verify event was received via subscription
    let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout")
        .expect("recv failed");
    assert_eq!(event.model_name, "TestUser");
    assert!(matches!(
        event.event_type,
        atomo::events::EventType::Created
    ));
}

#[tokio::test]
#[ignore]
async fn test_find_many_returns_created_records() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("Bob"));
    client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");

    let results = client
        .find_many("TestUser", &[], &[], None, None, &[])
        .await
        .expect("find failed");
    assert!(!results.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_soft_delete_hides_records() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("ToDelete"));
    let record = client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");

    // Delete (soft)
    use atomo::query::{WhereClause, WhereOperator};
    let id = record.get("id").cloned().unwrap_or(Value::Null);
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];
    let count = client
        .delete_many("TestUser", &where_clauses, None)
        .await
        .expect("delete failed");
    assert_eq!(count, 1);

    // Should not appear in find_many
    let results = client
        .find_many("TestUser", &where_clauses, &[], None, None, &[])
        .await
        .expect("find failed");
    assert!(results.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_event_store_replay() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("EventTest"));
    client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");

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
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("Before"));
    let record = client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");
    let id = record.get("id").cloned().unwrap();

    let mut rx = client.subscribe("TestUser", &[], &[]).await;

    use atomo::query::{WhereClause, WhereOperator};
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];
    let mut update_data = HashMap::new();
    update_data.insert("name".to_string(), json!("After"));
    let updated = client
        .update_many("TestUser", &where_clauses, &update_data, &[], None)
        .await
        .expect("update failed");
    assert_eq!(updated[0].get("name").unwrap(), &json!("After"));

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout")
        .expect("recv failed");
    assert!(matches!(
        event.event_type,
        atomo::events::EventType::Updated
    ));
}

#[tokio::test]
#[ignore]
async fn test_delete_emits_deleted_event() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("DeleteMe"));
    let record = client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");
    let id = record.get("id").cloned().unwrap();

    let mut rx = client.subscribe("TestUser", &[], &[]).await;

    use atomo::query::{WhereClause, WhereOperator};
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];
    let count = client
        .delete_many("TestUser", &where_clauses, None)
        .await
        .expect("delete failed");
    assert_eq!(count, 1);

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout")
        .expect("recv failed");
    assert!(matches!(
        event.event_type,
        atomo::events::EventType::Deleted
    ));
}

#[tokio::test]
#[ignore]
async fn test_count_excludes_soft_deleted() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data1 = HashMap::new();
    data1.insert("name".to_string(), json!("CountA"));
    let rec1 = client
        .create("TestUser", &data1, &[], None)
        .await
        .expect("create failed");

    let mut data2 = HashMap::new();
    data2.insert("name".to_string(), json!("CountB"));
    client
        .create("TestUser", &data2, &[], None)
        .await
        .expect("create failed");

    let before = client.count("TestUser", &[]).await.expect("count failed");
    assert!(before >= 2);

    use atomo::query::{WhereClause, WhereOperator};
    let id = rec1.get("id").cloned().unwrap();
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];
    client
        .delete_many("TestUser", &where_clauses, None)
        .await
        .expect("delete failed");

    let after = client.count("TestUser", &[]).await.expect("count failed");
    assert_eq!(after, before - 1);
}

#[tokio::test]
#[ignore]
async fn test_find_unique_by_id() {
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("UniqueFind"));
    let record = client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");
    let id = record.get("id").cloned().unwrap();

    use atomo::query::{WhereClause, WhereOperator};
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];
    let found = client
        .find_unique("TestUser", &where_clauses, &[])
        .await
        .expect("find_unique failed");
    assert!(found.is_some());
    assert_eq!(found.unwrap().get("name").unwrap(), &json!("UniqueFind"));
}

#[tokio::test]
#[ignore]
async fn test_restore_and_hard_delete() {
    use atomo::query::{WhereClause, WhereOperator};
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("Restorable"));
    let record = client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");
    let id = record.get("id").cloned().unwrap();
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];

    // Soft delete -> hidden
    assert_eq!(
        client
            .delete_many("TestUser", &where_clauses, None)
            .await
            .unwrap(),
        1
    );
    assert!(client
        .find_many("TestUser", &where_clauses, &[], None, None, &[])
        .await
        .unwrap()
        .is_empty());
    // ...but visible in the trash (find_deleted)
    assert_eq!(
        client
            .find_deleted("TestUser", &where_clauses, &[], None, None)
            .await
            .unwrap()
            .len(),
        1
    );

    // Restore -> visible again
    assert_eq!(
        client
            .restore_many("TestUser", &where_clauses, None)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        client
            .find_many("TestUser", &where_clauses, &[], None, None, &[])
            .await
            .unwrap()
            .len(),
        1
    );
    // ...and no longer in the trash
    assert!(client
        .find_deleted("TestUser", &where_clauses, &[], None, None)
        .await
        .unwrap()
        .is_empty());

    // Hard delete -> gone permanently (count of affected rows == 1)
    assert_eq!(
        client
            .hard_delete_many("TestUser", &where_clauses, None)
            .await
            .unwrap(),
        1
    );
    assert!(client
        .find_many("TestUser", &where_clauses, &[], None, None, &[])
        .await
        .unwrap()
        .is_empty());
    // A second restore affects 0 rows (the row no longer exists)
    assert_eq!(
        client
            .restore_many("TestUser", &where_clauses, None)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore]
async fn test_restore_emits_restored_event() {
    use atomo::query::{WhereClause, WhereOperator};
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("RestoreEvent"));
    let record = client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");
    let id = record.get("id").cloned().unwrap();
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];

    client
        .delete_many("TestUser", &where_clauses, None)
        .await
        .unwrap();

    let mut rx = client.subscribe("TestUser", &[], &[]).await;

    client
        .restore_many("TestUser", &where_clauses, None)
        .await
        .unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout waiting for Restored event")
        .expect("recv failed");
    assert_eq!(event.model_name, "TestUser");
    assert!(
        matches!(event.event_type, atomo::events::EventType::Restored),
        "expected Restored, got {:?}",
        event.event_type
    );
}

#[tokio::test]
#[ignore]
async fn test_hard_delete_emits_hard_deleted_event() {
    use atomo::query::{WhereClause, WhereOperator};
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("HardDeleteEvent"));
    let record = client
        .create("TestUser", &data, &[], None)
        .await
        .expect("create failed");
    let id = record.get("id").cloned().unwrap();
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];

    let mut rx = client.subscribe("TestUser", &[], &[]).await;

    client
        .hard_delete_many("TestUser", &where_clauses, None)
        .await
        .unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout waiting for HardDeleted event")
        .expect("recv failed");
    assert_eq!(event.model_name, "TestUser");
    assert!(
        matches!(event.event_type, atomo::events::EventType::HardDeleted),
        "expected HardDeleted, got {:?}",
        event.event_type
    );
}

#[tokio::test]
#[ignore]
async fn test_actor_persisted_and_replayed() {
    use atomo::query::{WhereClause, WhereOperator};
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("ActorTest"));
    let record = client
        .create("TestUser", &data, &[], Some("user-42"))
        .await
        .expect("create failed");
    let id = record.get("id").cloned().unwrap();

    // Update with a different actor
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id.clone(),
    }];
    let mut update_data = HashMap::new();
    update_data.insert("name".to_string(), json!("ActorTestUpdated"));
    client
        .update_many("TestUser", &where_clauses, &update_data, &[], Some("user-99"))
        .await
        .expect("update failed");

    // Replay all events and verify actors survived the round-trip
    let store = atomo::event_store::EventStore::new(client.db_pool().clone());
    let events = store.replay("TestUser", None).await.expect("replay failed");
    let actor_events: Vec<_> = events
        .iter()
        .filter(|e| e.data.get("name").map(|v| v.as_str()) == Some(Some("ActorTest"))
            || e.data.get("name").map(|v| v.as_str()) == Some(Some("ActorTestUpdated")))
        .collect();

    assert!(
        actor_events.len() >= 2,
        "expected at least a Created + Updated event, got {}",
        actor_events.len()
    );
    let created = actor_events
        .iter()
        .find(|e| matches!(e.event_type, atomo::events::EventType::Created))
        .expect("no Created event found");
    assert_eq!(
        created.actor.as_deref(),
        Some("user-42"),
        "Created event actor must be user-42"
    );
    let updated = actor_events
        .iter()
        .find(|e| matches!(e.event_type, atomo::events::EventType::Updated))
        .expect("no Updated event found");
    assert_eq!(
        updated.actor.as_deref(),
        Some("user-99"),
        "Updated event actor must be user-99"
    );

    // Also verify entity_history returns actor
    let id_str = id.as_str().unwrap();
    let history = store
        .entity_history("TestUser", id_str)
        .await
        .expect("entity_history failed");
    assert!(
        history.iter().any(|e| e.actor.as_deref() == Some("user-42")),
        "entity_history must include actor"
    );
}

#[tokio::test]
#[ignore]
async fn test_restore_and_hard_delete_events_persist_to_event_log() {
    use atomo::query::{WhereClause, WhereOperator};
    let schema = test_schema();
    let client = atomo::client::AtomoClient::builder()
        .database_url(
            std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/atomo_test".into()),
        )
        .enable_migrations(true)
        .build(&schema)
        .await
        .expect("Failed to build client");

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("EventLogPersist"));
    let record = client
        .create("TestUser", &data, &[], Some("actor-a"))
        .await
        .expect("create failed");
    let id = record.get("id").cloned().unwrap();
    let id_str = id.as_str().unwrap().to_string();
    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: id,
    }];

    // Soft delete → restore → hard delete
    client
        .delete_many("TestUser", &where_clauses, Some("actor-b"))
        .await
        .unwrap();
    client
        .restore_many("TestUser", &where_clauses, Some("actor-c"))
        .await
        .unwrap();
    client
        .hard_delete_many("TestUser", &where_clauses, Some("actor-d"))
        .await
        .unwrap();

    // Replay entity history — all four lifecycle events should be persisted
    let store = atomo::event_store::EventStore::new(client.db_pool().clone());
    let history = store
        .entity_history("TestUser", &id_str)
        .await
        .expect("entity_history failed");

    let types: Vec<_> = history.iter().map(|e| format!("{:?}", e.event_type)).collect();
    assert!(
        types.contains(&"Created".to_string()),
        "missing Created in {types:?}"
    );
    assert!(
        types.contains(&"Deleted".to_string()),
        "missing Deleted in {types:?}"
    );
    assert!(
        types.contains(&"Restored".to_string()),
        "missing Restored in {types:?}"
    );
    assert!(
        types.contains(&"HardDeleted".to_string()),
        "missing HardDeleted in {types:?}"
    );

    // Verify actors on each event
    let actors: Vec<_> = history
        .iter()
        .map(|e| (format!("{:?}", e.event_type), e.actor.clone()))
        .collect();
    assert!(
        actors.contains(&("Created".to_string(), Some("actor-a".to_string()))),
        "Created actor mismatch in {actors:?}"
    );
    assert!(
        actors.contains(&("Deleted".to_string(), Some("actor-b".to_string()))),
        "Deleted actor mismatch in {actors:?}"
    );
    assert!(
        actors.contains(&("Restored".to_string(), Some("actor-c".to_string()))),
        "Restored actor mismatch in {actors:?}"
    );
    assert!(
        actors.contains(&("HardDeleted".to_string(), Some("actor-d".to_string()))),
        "HardDeleted actor mismatch in {actors:?}"
    );
}
