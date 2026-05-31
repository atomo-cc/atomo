//! CRM dogfood: load the REAL services/crm-service/schema.ts through the platform and run a
//! realistic flow (Company -> Contact -> Deal -> stage move -> relationship). The flagship is
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
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn eq(field: &str, value: Value) -> WhereClause {
    WhereClause { field: field.to_string(), operator: WhereOperator::Equals, value }
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
    let company_id = company.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // 2. Contact in that company
    let contact = c
        .create(
            "Contact",
            &rec(&[
                ("firstName", json!("Ada")),
                ("lastName", json!("Lovelace")),
                ("email", json!("ada@acme.com")),
                ("companyId", json!(company_id)),
            ]),
            &[],
            None,
        )
        .await
        .expect("create Contact");
    let contact_id = contact.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // 3. Deal referencing the contact — stage is an enum (DealStage), value is numeric.
    let deal = c
        .create(
            "Deal",
            &rec(&[
                ("title", json!("Acme renewal")),
                ("value", json!(50000)),
                ("stage", json!("lead")),
                ("position", json!(0)),
                ("contactId", json!(contact_id)),
            ]),
            &[],
            None,
        )
        .await
        .expect("create Deal (enum stage + numeric value)");
    let deal_id = deal.get("id").and_then(|v| v.as_str()).unwrap().to_string();
    assert_eq!(deal.get("stage").and_then(|v| v.as_str()), Some("lead"), "stage round-trips as a string");

    // 4. Move the deal across stages (the Kanban core operation).
    let moved = c
        .update_many("Deal", &[eq("id", json!(deal_id))], &rec(&[("stage", json!("qualified"))]), &[], None)
        .await
        .expect("move Deal stage");
    assert_eq!(moved.len(), 1, "exactly one deal updated");
    assert_eq!(moved[0].get("stage").and_then(|v| v.as_str()), Some("qualified"), "stage advanced");

    // Update-aware validation: the stage-only patch above succeeded despite `title` (required)
    // being absent. But setting title to empty in a patch must still be rejected.
    let bad_update = c
        .update_many("Deal", &[eq("id", json!(deal_id))], &rec(&[("title", json!(""))]), &[], None)
        .await;
    assert!(bad_update.is_err(), "empty title in an update patch must fail validation, got {:?}", bad_update);

    // 5. Relationship: the contact's deals (hasMany via contactId).
    let deals = c
        .find_many("Deal", &[eq("contactId", json!(contact_id))], &[], None, None, &[])
        .await
        .expect("query deals by contact");
    assert_eq!(deals.len(), 1, "contact should have exactly one deal");

    // 5b. Nested includes via resolve_includes: contact.company (belongsTo) + contact.deals
    //     (hasMany). This is the real relationship-resolution path, not a flat FK query.
    let with_rels = c
        .find_many("Contact", &[eq("id", json!(contact_id))], &[], None, None, &["company".into(), "deals".into()])
        .await
        .expect("query contact with includes");
    let contact_full = &with_rels[0];
    // belongsTo: company should be nested as an object with the right name.
    let company_rel = contact_full.get("company");
    assert!(
        company_rel.and_then(|v| v.get("name")).and_then(|v| v.as_str()) == Some("Acme Inc"),
        "contact.company (belongsTo) should resolve to the company object, got: {:?}", company_rel
    );
    // hasMany: deals should be a nested array containing the deal.
    let deals_rel = contact_full.get("deals").and_then(|v| v.as_array());
    assert!(
        deals_rel.is_some_and(|a| a.len() == 1),
        "contact.deals (hasMany) should resolve to a 1-element array, got: {:?}", contact_full.get("deals")
    );

    // 6. Validation is enforced in the DATA layer (not just GraphQL). The CRM declares
    //    title: required and Company name: required — empty values must be rejected here.
    let bad_deal = c
        .create("Deal", &rec(&[("title", json!("")), ("value", json!(1)), ("stage", json!("lead")), ("position", json!(0)), ("contactId", json!(contact_id))]), &[], None)
        .await;
    assert!(bad_deal.is_err(), "empty required title should fail validation, got {:?}", bad_deal);

    let bad_company = c
        .create("Company", &rec(&[("name", json!(""))]), &[], None)
        .await;
    assert!(bad_company.is_err(), "empty required name should fail validation");

    // 7. Pagination + orderBy via CRM. Add a second, higher-value deal, then order by value DESC
    //    with limit 1 → the bigger deal comes first.
    c.create("Deal", &rec(&[("title", json!("Big deal")), ("value", json!(99000)), ("stage", json!("lead")), ("position", json!(1)), ("contactId", json!(contact_id))]), &[], None)
        .await
        .expect("create second Deal");
    let top = c
        .find_many("Deal", &[], &[("value".into(), OrderDirection::Desc)], Some(1), None, &[])
        .await
        .expect("orderBy+limit query");
    assert_eq!(top.len(), 1, "limit 1 returns one row");
    assert_eq!(top[0].get("title").and_then(|v| v.as_str()), Some("Big deal"), "orderBy value DESC returns the biggest first");
    // offset 1 → the second (smaller) deal.
    let next = c
        .find_many("Deal", &[], &[("value".into(), OrderDirection::Desc)], Some(1), Some(1), &[])
        .await
        .expect("orderBy+offset query");
    assert_eq!(next[0].get("title").and_then(|v| v.as_str()), Some("Acme renewal"), "offset paginates");

    // 8. Soft-delete lifecycle via CRM: delete the first Deal → hidden from find_many, present in
    //    trash, then restore → visible again.
    let deleted = c.delete_many("Deal", &[eq("id", json!(deal_id))], None).await.expect("soft delete");
    assert_eq!(deleted, 1, "one deal soft-deleted");
    let live = c.find_many("Deal", &[], &[], None, None, &[]).await.unwrap();
    assert!(!live.iter().any(|d| d.get("id").and_then(|v| v.as_str()) == Some(deal_id.as_str())), "soft-deleted deal hidden");
    let trashed = c.find_deleted("Deal", &[], &[], None, None).await.expect("trash view");
    assert!(trashed.iter().any(|d| d.get("id").and_then(|v| v.as_str()) == Some(deal_id.as_str())), "deleted deal in trash");
    let restored = c.restore_many("Deal", &[eq("id", json!(deal_id))]).await.expect("restore");
    assert_eq!(restored, 1, "one deal restored");
    let live2 = c.find_many("Deal", &[], &[], None, None, &[]).await.unwrap();
    assert!(live2.iter().any(|d| d.get("id").and_then(|v| v.as_str()) == Some(deal_id.as_str())), "restored deal visible again");

    // Cleanup the tables this test generated.
    for t in ["deal", "contact", "company", "activity"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", t)).execute(atomo.db_pool()).await.ok();
    }
}


// Phase C3: event sourcing — a Deal's full lifecycle (Created → stage Updated → Deleted) is
// persisted to event_log and reconstructable via EventStore::entity_history. This relies on
// every event carrying the id (the B2 delete-event fix is what makes Deleted show up here).
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

    let contact = c.create("Contact", &rec(&[("firstName", json!("E")), ("lastName", json!("S")), ("email", json!("e@s.com"))]), &[], None).await.unwrap();
    let cid = contact.get("id").and_then(|v| v.as_str()).unwrap().to_string();
    let deal = c.create("Deal", &rec(&[("title", json!("Hist")), ("value", json!(10)), ("stage", json!("lead")), ("position", json!(0)), ("contactId", json!(cid))]), &[], None).await.unwrap();
    let did = deal.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    let by_id = |id: &str| vec![WhereClause { field: "id".into(), operator: WhereOperator::Equals, value: json!(id) }];
    c.update_many("Deal", &by_id(&did), &rec(&[("stage", json!("qualified"))]), &[], None).await.unwrap();
    c.update_many("Deal", &by_id(&did), &rec(&[("stage", json!("won"))]), &[], None).await.unwrap();
    c.delete_many("Deal", &by_id(&did), None).await.unwrap();

    // Reconstruct the Deal's history from the event store.
    let store = atomo::event_store::EventStore::new(atomo.db_pool().clone());
    let history = store.entity_history("Deal", &did).await.expect("entity_history");
    use atomo::events::EventType;
    let types: Vec<EventType> = history.iter().map(|e| e.event_type).collect();
    assert_eq!(
        types,
        vec![EventType::Created, EventType::Updated, EventType::Updated, EventType::Deleted],
        "Deal history must replay Created → Updated → Updated → Deleted, got {:?}", types
    );
    // The Deleted event must carry the id (B2 fix) — that's why it appears in entity_history.
    assert_eq!(history.last().unwrap().data.get("id").and_then(|v| v.as_str()), Some(did.as_str()));

    for t in ["deal", "contact", "company", "activity"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", t)).execute(atomo.db_pool()).await.ok();
    }
}
