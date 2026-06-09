//! CRM dogfood: load the REAL services/crm-service/schema.ts through the platform and run a
//! realistic flow (Company -> Contact -> Deal -> status move -> relationship). The flagship is
//! supposed to drive the platform; this is the first test that actually eats that dog food.
//! Requires Postgres via DATABASE_URL. Run: cargo test -p atomo --test crm_dogfood -- --ignored

use std::collections::HashMap;

use atomo::query::{OrderDirection, WhereClause, WhereOperator};
use serde_json::{json, Value};

fn crm_schema_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../services/crm-service/schema.ts")
        .to_string_lossy()
        .into_owned()
}

fn rec(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn eq(field: &str, value: Value) -> WhereClause {
    WhereClause {
        field: field.to_string(),
        operator: WhereOperator::Equals,
        value,
    }
}

#[tokio::test]
#[ignore]
async fn crm_schema_drives_the_platform() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let atomo = atomo::Atomo::builder()
        .schema_file(crm_schema_path())
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .expect("the real CRM schema should build + migrate");
    let c = atomo.client();

    // 1. Company
    let company = c
        .create("Company", &rec(&[("name", json!("Acme Inc"))]), &[], None)
        .await
        .expect("create Company");
    let company_id = company
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    // 2. Contact in that company
    let contact = c
        .create(
            "Contact",
            &rec(&[
                ("name", json!("Ada Lovelace")),
                ("email", json!("ada@acme.com")),
                ("companyId", json!(company_id)),
            ]),
            &[],
            None,
        )
        .await
        .expect("create Contact");
    let contact_id = contact
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    // 3. Deal referencing the contact
    let deal = c
        .create(
            "Deal",
            &rec(&[
                ("title", json!("Acme renewal")),
                ("value", json!(50000)),
                ("status", json!("open")),
                ("contactId", json!(contact_id)),
            ]),
            &[],
            None,
        )
        .await
        .expect("create Deal");
    let deal_id = deal.get("id").and_then(|v| v.as_str()).unwrap().to_string();
    assert_eq!(
        deal.get("status").and_then(|v| v.as_str()),
        Some("open"),
        "status round-trips as a string"
    );

    // 4. Move the deal status (the pipeline core operation).
    let moved = c
        .update_many(
            "Deal",
            &[eq("id", json!(deal_id))],
            &rec(&[("status", json!("won"))]),
            &[],
            None,
        )
        .await
        .expect("move Deal status");
    assert_eq!(moved.len(), 1, "exactly one deal updated");
    assert_eq!(
        moved[0].get("status").and_then(|v| v.as_str()),
        Some("won"),
        "status advanced"
    );

    // Update-aware validation: the stage-only patch above succeeded despite `title` (required)
    // being absent. But setting title to empty in a patch must still be rejected.
    let bad_update = c
        .update_many(
            "Deal",
            &[eq("id", json!(deal_id))],
            &rec(&[("title", json!(""))]),
            &[],
            None,
        )
        .await;
    assert!(
        bad_update.is_err(),
        "empty title in an update patch must fail validation, got {:?}",
        bad_update
    );

    // 5. Relationship: the contact's deals (hasMany via contactId).
    let deals = c
        .find_many(
            "Deal",
            &[eq("contactId", json!(contact_id))],
            &[],
            None,
            None,
            &[],
        )
        .await
        .expect("query deals by contact");
    assert_eq!(deals.len(), 1, "contact should have exactly one deal");

    // 5b. Nested includes via resolve_includes: contact.company (belongsTo).
    let with_rels = c
        .find_many(
            "Contact",
            &[eq("id", json!(contact_id))],
            &[],
            None,
            None,
            &["company".into()],
        )
        .await
        .expect("query contact with includes");
    let contact_full = &with_rels[0];
    let company_rel = contact_full.get("company");
    assert!(
        company_rel
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            == Some("Acme Inc"),
        "contact.company (belongsTo) should resolve, got: {:?}",
        company_rel
    );

    // 6. Validation: required title → empty must be rejected.
    let bad_deal = c
        .create(
            "Deal",
            &rec(&[
                ("title", json!("")),
                ("value", json!(1)),
                ("status", json!("open")),
                ("contactId", json!(contact_id)),
            ]),
            &[],
            None,
        )
        .await;
    assert!(
        bad_deal.is_err(),
        "empty required title should fail validation, got {:?}",
        bad_deal
    );

    let bad_company = c
        .create("Company", &rec(&[("name", json!(""))]), &[], None)
        .await;
    assert!(
        bad_company.is_err(),
        "empty required name should fail validation"
    );

    // 7. Pagination + orderBy: add a second deal, order by value DESC.
    c.create(
        "Deal",
        &rec(&[
            ("title", json!("Big deal")),
            ("value", json!(99000)),
            ("status", json!("open")),
            ("contactId", json!(contact_id)),
        ]),
        &[],
        None,
    )
    .await
    .expect("create second Deal");
    let top = c
        .find_many(
            "Deal",
            &[],
            &[("value".into(), OrderDirection::Desc)],
            Some(1),
            None,
            &[],
        )
        .await
        .expect("orderBy+limit query");
    assert_eq!(top.len(), 1, "limit 1 returns one row");
    assert_eq!(
        top[0].get("title").and_then(|v| v.as_str()),
        Some("Big deal"),
        "orderBy value DESC returns the biggest first"
    );
    // offset 1 → the second (smaller) deal.
    let next = c
        .find_many(
            "Deal",
            &[],
            &[("value".into(), OrderDirection::Desc)],
            Some(1),
            Some(1),
            &[],
        )
        .await
        .expect("orderBy+offset query");
    assert_eq!(
        next[0].get("title").and_then(|v| v.as_str()),
        Some("Acme renewal"),
        "offset paginates"
    );

    // 7b. Read-cache conformance.
    let before = c
        .find_many("Deal", &[], &[], None, None, &[])
        .await
        .unwrap();
    c.create(
        "Deal",
        &rec(&[
            ("title", json!("Cache deal")),
            ("value", json!(5)),
            ("status", json!("open")),
            ("contactId", json!(contact_id)),
        ]),
        &[],
        None,
    )
    .await
    .expect("create cache deal");
    let after = c
        .find_many("Deal", &[], &[], None, None, &[])
        .await
        .unwrap();
    assert_eq!(
        after.len(),
        before.len() + 1,
        "create must invalidate the read cache"
    );

    // 8. Soft-delete lifecycle.
    let deleted = c
        .delete_many("Deal", &[eq("id", json!(deal_id))], None)
        .await
        .expect("soft delete");
    assert_eq!(deleted, 1, "one deal soft-deleted");
    let live = c
        .find_many("Deal", &[], &[], None, None, &[])
        .await
        .unwrap();
    assert!(
        !live
            .iter()
            .any(|d| d.get("id").and_then(|v| v.as_str()) == Some(deal_id.as_str())),
        "soft-deleted deal hidden"
    );
    let trashed = c
        .find_deleted("Deal", &[], &[], None, None)
        .await
        .expect("trash view");
    assert!(
        trashed
            .iter()
            .any(|d| d.get("id").and_then(|v| v.as_str()) == Some(deal_id.as_str())),
        "deleted deal in trash"
    );
    let restored = c
        .restore_many("Deal", &[eq("id", json!(deal_id))], None)
        .await
        .expect("restore");
    assert_eq!(restored, 1, "one deal restored");
    let live2 = c
        .find_many("Deal", &[], &[], None, None, &[])
        .await
        .unwrap();
    assert!(
        live2
            .iter()
            .any(|d| d.get("id").and_then(|v| v.as_str()) == Some(deal_id.as_str())),
        "restored deal visible again"
    );

    // Cleanup the tables this test generated.
    for t in ["deals", "contacts", "companies", "activities"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", t))
            .execute(atomo.db_pool())
            .await
            .ok();
    }
}

#[tokio::test]
#[ignore]
async fn crm_deal_event_history_replays() {
    use atomo::query::{WhereClause, WhereOperator};
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let atomo = atomo::Atomo::builder()
        .schema_file(crm_schema_path())
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let c = atomo.client();

    let contact = c
        .create(
            "Contact",
            &rec(&[
                ("name", json!("Eve S")),
                ("email", json!("e@s.com")),
            ]),
            &[],
            None,
        )
        .await
        .unwrap();
    let cid = contact
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    let deal = c
        .create(
            "Deal",
            &rec(&[
                ("title", json!("Hist")),
                ("value", json!(10)),
                ("status", json!("open")),
                ("contactId", json!(cid)),
            ]),
            &[],
            None,
        )
        .await
        .unwrap();
    let did = deal.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    let by_id = |id: &str| {
        vec![WhereClause {
            field: "id".into(),
            operator: WhereOperator::Equals,
            value: json!(id),
        }]
    };
    c.update_many(
        "Deal",
        &by_id(&did),
        &rec(&[("status", json!("won"))]),
        &[],
        None,
    )
    .await
    .unwrap();
    c.delete_many("Deal", &by_id(&did), None).await.unwrap();

    let store = atomo::event_store::EventStore::new(atomo.db_pool().clone());
    let history = store
        .entity_history("Deal", &did)
        .await
        .expect("entity_history");
    use atomo::events::EventType;
    let types: Vec<EventType> = history.iter().map(|e| e.event_type).collect();
    assert_eq!(
        types,
        vec![
            EventType::Created,
            EventType::Updated,
            EventType::Deleted
        ],
        "Deal history must replay Created → Updated → Deleted, got {:?}",
        types
    );
    assert_eq!(
        history
            .last()
            .unwrap()
            .data
            .get("id")
            .and_then(|v| v.as_str()),
        Some(did.as_str())
    );

    for t in ["deals", "contacts", "companies", "activities"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", t))
            .execute(atomo.db_pool())
            .await
            .ok();
    }
}

#[tokio::test]
#[ignore]
async fn data_layer_enforce_access_gates_by_role() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let atomo = atomo::Atomo::builder()
        .schema_file(crm_schema_path())
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let c = atomo.client();

    // Contact create requires sales|admin.
    assert!(
        c.enforce_access("Contact", "create", Some("Viewer"))
            .is_err(),
        "viewer denied create"
    );
    assert!(
        c.enforce_access("Contact", "create", Some("Sales")).is_ok(),
        "sales allowed create"
    );
    assert!(
        c.enforce_access("Contact", "create", None).is_err(),
        "anon needs auth"
    );
    // delete is admin only.
    assert!(
        c.enforce_access("Contact", "delete", Some("Sales"))
            .is_err(),
        "sales cannot delete"
    );
    assert!(
        c.enforce_access("Contact", "delete", Some("Admin")).is_ok(),
        "admin can delete"
    );
    // read is authenticated — any role allowed.
    assert!(
        c.enforce_access("Contact", "read", Some("Viewer")).is_ok(),
        "viewer can read"
    );

    for t in ["deals", "contacts", "companies", "activities"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", t))
            .execute(atomo.db_pool())
            .await
            .ok();
    }
}

// Unified-parser payoff: resolve_includes is now schema-driven. A relationship whose NAME
// differs from its target MODEL (`owner` -> `Contact`) resolves correctly — the old
// convention heuristic would have looked for a model literally named "Owner".
#[tokio::test]
#[ignore]
async fn schema_driven_include_resolves_renamed_relationship() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    // Deal.owner is a belongsTo Contact via ownerId — name (owner) != model (Contact).
    let schema_ts = r#"
export interface Contact {
  id: string;
  name: string;
}
export interface Deal {
  id: string;
  title: string;
  ownerId: string;
}
export const schema = { models: {
  Contact: { tableName: 'contacts' },
  Deal: { tableName: 'deals', relationships: { owner: { type: 'belongsTo', model: 'Contact', foreignKey: 'ownerId' } } }
} };
export default schema;
"#;
    let atomo = atomo::Atomo::builder()
        .schema_content(schema_ts)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let c = atomo.client();

    let owner = c
        .create("Contact", &rec(&[("name", json!("Ada"))]), &[], None)
        .await
        .unwrap();
    let owner_id = owner
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    let deal = c
        .create(
            "Deal",
            &rec(&[("title", json!("D1")), ("ownerId", json!(owner_id))]),
            &[],
            None,
        )
        .await
        .unwrap();
    let did = deal.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    let with_owner = c
        .find_many(
            "Deal",
            &[eq("id", json!(did))],
            &[],
            None,
            None,
            &["owner".into()],
        )
        .await
        .unwrap();
    let owner_rel = with_owner[0].get("owner");
    assert!(
        owner_rel
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            == Some("Ada"),
        "owner (belongsTo Contact via ownerId) should resolve despite name != model, got: {:?}",
        owner_rel
    );

    for t in ["deals", "contacts"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", t))
            .execute(atomo.db_pool())
            .await
            .ok();
    }
}
