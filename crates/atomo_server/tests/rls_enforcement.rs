//! Phase-3 RLS enforcement, against a real Postgres. Proves the database itself blocks
//! cross-tenant reads/writes once `ensure_rls_policies` is applied and a tenant is bound
//! via `bind_tenant` — i.e. a forgotten app-layer `WHERE tenant_id = …` cannot leak.
//!
//! Requires Postgres via DATABASE_URL. Uses a disposable, namespaced table (dropped at the
//! end) so it never touches application data.
//! Run: cargo test -p atomo_server --test rls_enforcement -- --ignored

use sqlx::Row;

const TABLE: &str = "rls_test_widgets";

async fn names_visible_to(pool: &sqlx::PgPool, tenant: Option<&str>) -> Vec<String> {
    let mut tx = pool.begin().await.unwrap();
    atomo_server::rls::bind_tenant(&mut tx, tenant).await.unwrap();
    let rows = sqlx::query(&format!("SELECT name FROM {TABLE} ORDER BY name"))
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rows.iter().map(|r| r.get::<String, _>(0)).collect()
}

#[tokio::test]
#[ignore]
async fn rls_blocks_cross_tenant_reads_and_writes() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    // Superusers and BYPASSRLS roles bypass even FORCE RLS — enforcement can't be demonstrated
    // as such a role. Skip with a clear message rather than fail spuriously.
    let is_super: bool = sqlx::query("SELECT current_setting('is_superuser') = 'on'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    if is_super {
        eprintln!(
            "SKIP rls_enforcement: connected as a superuser — RLS is bypassed. \
             Run as a non-superuser app role to verify enforcement."
        );
        return;
    }

    // Disposable table with the standard nullable tenant_id column.
    sqlx::query(&format!("DROP TABLE IF EXISTS {TABLE}"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&format!(
        "CREATE TABLE {TABLE} (id TEXT PRIMARY KEY, tenant_id TEXT, name TEXT)"
    ))
    .execute(&pool)
    .await
    .unwrap();
    // Two tenants + one NULL-tenant (global/unscoped) row.
    sqlx::query(&format!(
        "INSERT INTO {TABLE} (id, tenant_id, name) VALUES \
         ('a1','tenant-a','A1'), ('b1','tenant-b','B1'), ('g1', NULL, 'GLOBAL')"
    ))
    .execute(&pool)
    .await
    .unwrap();

    // Apply RLS via the real production code path.
    atomo_server::rls::ensure_rls_policies(&pool, &[TABLE.to_string()], true)
        .await
        .unwrap();

    // Tenant A sees only A's row + the NULL-tenant row; never B's.
    assert_eq!(
        names_visible_to(&pool, Some("tenant-a")).await,
        vec!["A1".to_string(), "GLOBAL".to_string()],
        "tenant-a must see only its own + NULL-tenant rows"
    );
    // Tenant B symmetrically.
    assert_eq!(
        names_visible_to(&pool, Some("tenant-b")).await,
        vec!["B1".to_string(), "GLOBAL".to_string()],
        "tenant-b must see only its own + NULL-tenant rows"
    );
    // No tenant bound → only NULL-tenant rows are visible (the safe default).
    assert_eq!(
        names_visible_to(&pool, None).await,
        vec!["GLOBAL".to_string()],
        "an unbound request must see only NULL-tenant rows"
    );

    // WITH CHECK: tenant A cannot insert a row stamped as tenant B.
    let mut tx = pool.begin().await.unwrap();
    atomo_server::rls::bind_tenant(&mut tx, Some("tenant-a"))
        .await
        .unwrap();
    let smuggle = sqlx::query(&format!(
        "INSERT INTO {TABLE} (id, tenant_id, name) VALUES ('x1','tenant-b','SMUGGLED')"
    ))
    .execute(&mut *tx)
    .await;
    assert!(
        smuggle.is_err(),
        "WITH CHECK must reject writing another tenant's row"
    );
    let _ = tx.rollback().await;

    // Cleanup.
    sqlx::query(&format!("DROP TABLE IF EXISTS {TABLE}"))
        .execute(&pool)
        .await
        .unwrap();
}
