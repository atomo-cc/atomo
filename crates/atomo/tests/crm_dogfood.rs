//! CRM dogfood: load the REAL services/crm-service/schema.ts through the platform and run a
//! realistic flow (Company -> Contact -> Deal -> stage move -> relationship). The flagship is
//! supposed to drive the platform; this is the first test that actually eats that dog food.
//! Requires Postgres via DATABASE_URL. Run: cargo test -p atomo --test crm_dogfood -- --ignored

use std::collections::HashMap;

use atomo::query::{WhereClause, WhereOperator};
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

    // 5. Relationship: the contact's deals (hasMany via contactId).
    let deals = c
        .find_many("Deal", &[eq("contactId", json!(contact_id))], &[], None, None, &[])
        .await
        .expect("query deals by contact");
    assert_eq!(deals.len(), 1, "contact should have exactly one deal");

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

    // Cleanup the tables this test generated.
    for t in ["deal", "contact", "company", "activity"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", t)).execute(atomo.db_pool()).await.ok();
    }
}
