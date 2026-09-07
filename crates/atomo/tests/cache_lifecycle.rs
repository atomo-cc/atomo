//! Real core read-path cache regression coverage; requires an isolated DATABASE_URL.
use atomo::{cache::CacheConfig, client::with_tenant_scope};
use serde_json::json;
use std::collections::HashMap;
const SCHEMA:&str="export interface CacheOwner { id: string; name: string; }\nexport interface CacheProbe { id: string; title: string; ownerId?: string; }\nexport const schema = { models: { CacheOwner: { tableName: 'cache_lifecycle_owners' }, CacheProbe: { tableName: 'cache_lifecycle_probes', relationships: { owner: { type: 'belongsTo', model: 'CacheOwner', foreignKey: 'ownerId' } } } } }; export default schema;";
#[tokio::test]
#[ignore]
async fn cache_wired_keys_refresh_writes_and_bypass() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let app = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(&url)
        .enable_migrations(true)
        .cache_config(CacheConfig::default())
        .build()
        .await
        .unwrap();
    let c = app.client();
    sqlx::query("DELETE FROM cache_lifecycle_probes")
        .execute(c.db_pool())
        .await
        .unwrap();
    let owner = c
        .create(
            "CacheOwner",
            &HashMap::from([("name".into(), json!("owner"))]),
            &[],
            None,
        )
        .await
        .unwrap();
    c.create(
        "CacheProbe",
        &HashMap::from([
            ("title".into(), json!("first")),
            ("ownerId".into(), owner["id"].clone()),
        ]),
        &[],
        None,
    )
    .await
    .unwrap();
    let before = c.cache_status().await.metrics;
    let first = with_tenant_scope(
        Some("scope-a".into()),
        c.find_many("CacheProbe", &[], &[], None, None, &[]),
    )
    .await
    .unwrap();
    assert_eq!(first.len(), 1);
    with_tenant_scope(
        Some("scope-a".into()),
        c.find_many("CacheProbe", &[], &[], None, None, &[]),
    )
    .await
    .unwrap();
    let after = c.cache_status().await.metrics;
    assert_eq!(after.hits, before.hits + 1);
    with_tenant_scope(
        Some("scope-b".into()),
        c.find_many("CacheProbe", &[], &[], None, None, &[]),
    )
    .await
    .unwrap();
    assert_eq!(
        c.cache_status().await.metrics.misses,
        after.misses + 1,
        "tenant identity partitions identical query keys"
    );
    // Include graphs bypass and cannot collide with a cached scalar list or hold nested fill locks.
    let included = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        c.find_many("CacheProbe", &[], &[], None, None, &["owner".into()]),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(included[0]["owner"]["name"], json!("owner"));
    assert!(!first[0].contains_key("owner"));
    assert_eq!(c.cache_status().await.metrics.relational_bypasses, 1);
    c.create(
        "CacheProbe",
        &HashMap::from([("title".into(), json!("second"))]),
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        with_tenant_scope(
            Some("scope-a".into()),
            c.find_many("CacheProbe", &[], &[], None, None, &[])
        )
        .await
        .unwrap()
        .len(),
        2
    );
    assert_eq!(c.count("CacheProbe", &[]).await.unwrap(), 2);
    assert_eq!(
        c.find_many("CacheProbe", &[], &[], Some(1), Some(1), &[])
            .await
            .unwrap()
            .len(),
        1
    );
    let bypass = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(&url)
        .cache_config(CacheConfig {
            multi_instance: true,
            ..Default::default()
        })
        .build()
        .await
        .unwrap();
    bypass
        .client()
        .find_many("CacheProbe", &[], &[], None, None, &[])
        .await
        .unwrap();
    assert_eq!(bypass.client().cache_status().await.metrics.entries, 0);
    sqlx::query("DROP TABLE cache_lifecycle_probes, cache_lifecycle_owners")
        .execute(c.db_pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn unknown_model_override_is_rejected_before_database_connection() {
    let mut config = CacheConfig::default();
    config.models.insert(
        "TypoModel".into(),
        atomo::cache::CachePolicyOverride {
            enabled: Some(false),
            ..Default::default()
        },
    );
    let result = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url("postgresql://invalid:1/unreachable")
        .cache_config(config)
        .build()
        .await;
    let error = result.err().expect("unknown model must reject startup");
    assert!(error.to_string().contains("unknown model"), "{error}");
}
