//! Phase-3 RLS enforcement, against a real Postgres. Proves the database itself blocks
//! cross-tenant reads/writes once `ensure_rls_policies` is applied and a tenant is bound
//! via `bind_tenant` — i.e. a forgotten app-layer `WHERE tenant_id = …` cannot leak.
//!
//! Requires Postgres via DATABASE_URL. Uses a disposable, namespaced table (dropped at the
//! end) so it never touches application data.
//! Run: cargo test -p atomo_server --test rls_enforcement -- --ignored

use sqlx::Row;

async fn policy_snapshot(
    pool: &sqlx::PgPool,
    table: &str,
) -> (bool, bool, Vec<(String, Option<String>, Option<String>)>) {
    let flags: (bool, bool) = sqlx::query_as(
        "SELECT relrowsecurity, relforcerowsecurity FROM pg_class WHERE oid = $1::regclass",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .unwrap();
    let policies = sqlx::query_as(
        "SELECT policyname::text, qual, with_check FROM pg_policies \
         WHERE schemaname = current_schema() AND tablename = $1 ORDER BY policyname",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap();
    (flags.0, flags.1, policies)
}

#[tokio::test]
#[ignore]
async fn policy_installation_rejects_a_missing_table() {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let table = format!("rls_missing_{}", uuid::Uuid::new_v4().simple());
    let result =
        atomo_server::rls::ensure_rls_policies(&pool, std::slice::from_ref(&table), true).await;
    let error = result.expect_err("missing table must abort policy installation");
    assert!(error.to_string().contains(&table), "{error:#}");
}

#[tokio::test]
#[ignore]
async fn policy_installation_rolls_back_every_table_and_preserves_old_policies() {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    const GOOD: &str = "rls_install_a_good";
    const BAD: &str = "rls_install_z_bad";
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS rls_install_a_good, rls_install_z_bad; \
         CREATE TABLE rls_install_a_good (id TEXT PRIMARY KEY, tenant_id TEXT); \
         CREATE TABLE rls_install_z_bad (id TEXT PRIMARY KEY); \
         ALTER TABLE rls_install_a_good ENABLE ROW LEVEL SECURITY; \
         CREATE POLICY atomo_tenant_isolation ON rls_install_a_good USING (false) WITH CHECK (false); \
         CREATE POLICY atomo_tenant_isolation ON rls_install_z_bad USING (false) WITH CHECK (false);",
    )
    .execute(&pool)
    .await
    .unwrap();
    let before = (
        policy_snapshot(&pool, GOOD).await,
        policy_snapshot(&pool, BAD).await,
    );
    let result =
        atomo_server::rls::ensure_rls_policies(&pool, &[GOOD.into(), BAD.into()], true).await;
    let after = (
        policy_snapshot(&pool, GOOD).await,
        policy_snapshot(&pool, BAD).await,
    );
    sqlx::raw_sql("DROP TABLE rls_install_a_good, rls_install_z_bad")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(after, before, "DDL failure must roll back the entire batch");
    assert!(result.is_err(), "missing tenant_id must abort installation");
    assert!(result.unwrap_err().to_string().contains(BAD));
}

#[tokio::test]
#[ignore]
async fn policy_installation_commits_all_tables_and_is_idempotent() {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    const TABLES: [&str; 2] = ["rls_install_first", "rls_install_second"];
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS rls_install_first, rls_install_second; \
         CREATE TABLE rls_install_first (id TEXT PRIMARY KEY, tenant_id TEXT); \
         CREATE TABLE rls_install_second (id TEXT PRIMARY KEY, tenant_id TEXT);",
    )
    .execute(&pool)
    .await
    .unwrap();
    let tables: Vec<_> = TABLES.iter().map(|s| s.to_string()).collect();
    atomo_server::rls::ensure_rls_policies(&pool, &tables, true)
        .await
        .unwrap();
    let first = (
        policy_snapshot(&pool, TABLES[0]).await,
        policy_snapshot(&pool, TABLES[1]).await,
    );
    atomo_server::rls::ensure_rls_policies(&pool, &tables, true)
        .await
        .unwrap();
    let second = (
        policy_snapshot(&pool, TABLES[0]).await,
        policy_snapshot(&pool, TABLES[1]).await,
    );
    sqlx::raw_sql("DROP TABLE rls_install_first, rls_install_second")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        first, second,
        "reinstallation must preserve the same policy"
    );
    for state in [first.0, first.1] {
        assert!(
            state.0 && state.1,
            "ENABLE and FORCE must both be committed"
        );
        assert_eq!(state.2.len(), 1);
        assert_eq!(state.2[0].0, atomo_server::rls::POLICY_NAME);
        assert!(state.2[0].1.as_ref().unwrap().contains("atomo.tenant_id"));
        assert_eq!(state.2[0].1, state.2[0].2);
    }
}

const TABLE: &str = "rls_test_widgets";

#[tokio::test]
#[ignore]
async fn schema_rls_batch_restores_required_and_global_policies_on_failure() {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE rls_schema_a (id TEXT, tenant_id TEXT); \
         CREATE TABLE rls_schema_b (id TEXT, tenant_id TEXT); \
         CREATE TABLE rls_schema_z (id TEXT, tenant_id TEXT); \
         ALTER TABLE rls_schema_a ENABLE ROW LEVEL SECURITY; \
         CREATE POLICY atomo_tenant_isolation ON rls_schema_a USING (false);",
    )
    .execute(&pool)
    .await
    .unwrap();
    let schema = atomo::schema::parse_typescript_schema(
        r#"
import { model, text } from '@atomo-cc/schema';
export const First = model('rls_schema_a', { fields: { id: text().id() } });
export const Second = model('rls_schema_b', { tenant: 'global', fields: { id: text().id() } });
export const Last = model('rls_schema_z', { fields: { id: text().id() } });
"#,
    )
    .unwrap();
    assert_eq!(schema.models.len(), 3);
    // Break the final table in this schema's iteration order, so the old
    // per-table installer necessarily commits earlier tables before failing.
    let broken = atomo::query::sql_builder::table_name_for(schema.models.values().last().unwrap());
    sqlx::query(&format!("ALTER TABLE {broken} DROP COLUMN tenant_id"))
        .execute(&pool)
        .await
        .unwrap();
    let before = (
        policy_snapshot(&pool, "rls_schema_a").await,
        policy_snapshot(&pool, "rls_schema_b").await,
        policy_snapshot(&pool, "rls_schema_z").await,
    );
    let mut changed = false;
    for _ in 0..2 {
        assert!(atomo_server::rls::ensure_schema_rls(&pool, &schema)
            .await
            .is_err());
        let after = (
            policy_snapshot(&pool, "rls_schema_a").await,
            policy_snapshot(&pool, "rls_schema_b").await,
            policy_snapshot(&pool, "rls_schema_z").await,
        );
        changed |= before != after;
    }
    sqlx::raw_sql("DROP TABLE rls_schema_a, rls_schema_b, rls_schema_z")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        !changed,
        "a failed schema installation must roll back the entire batch"
    );
}

#[tokio::test]
#[ignore]
async fn server_startup_propagates_policy_installation_errors() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS rls_boot_probes")
        .execute(&pool)
        .await
        .unwrap();
    let atomo = atomo::Atomo::builder()
        .database_url(&url)
        .schema_content(
            "export interface RlsBootProbe { id: string; } \
             export const schema = { models: { RlsBootProbe: { tableName: 'rls_boot_probes' } } };",
        )
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    // Simulate a damaged projection after migration but before boot installs RLS.
    sqlx::query("ALTER TABLE rls_boot_probes DROP COLUMN tenant_id")
        .execute(&pool)
        .await
        .unwrap();
    let config = atomo_server::ServerConfig {
        database_url: url,
        host: "127.0.0.1".into(),
        port: 0,
        enable_rls: true,
        // Even the explicit role-bypass flag must not suppress DDL failures.
        rls_allow_bypass_role: true,
        ..Default::default()
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        atomo_server::AtomoServer::from_atomo(config, atomo).run(),
    )
    .await;
    let state = policy_snapshot(&pool, "rls_boot_probes").await;
    sqlx::query("DROP TABLE rls_boot_probes")
        .execute(&pool)
        .await
        .unwrap();
    let error = result
        .expect("startup must return before a listener starts serving")
        .expect_err("policy installation failure must abort server startup");
    assert!(error.to_string().contains("rls_boot_probes"), "{error:#}");
    assert_eq!(state, (false, false, vec![]));
}

async fn names_visible_to(pool: &sqlx::PgPool, tenant: Option<&str>) -> Vec<String> {
    let mut tx = pool.begin().await.unwrap();
    atomo_server::rls::bind_tenant(&mut tx, tenant)
        .await
        .unwrap();
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

/// `check_connection_role` must agree with the role's actual attributes: it
/// refuses superuser/BYPASSRLS connections (RLS would be silently inert) and
/// only proceeds when `allow_bypass` downgrades that to a logged error.
#[tokio::test]
#[ignore]
async fn check_connection_role_refuses_bypass_roles() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let (role, rolsuper, rolbypassrls): (String, bool, bool) = sqlx::query_as(
        "SELECT rolname, rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let bypasses = rolsuper || rolbypassrls;
    eprintln!("connected role: {role} (super={rolsuper}, bypassrls={rolbypassrls})");

    if bypasses {
        let err = atomo_server::rls::check_connection_role(&pool, false)
            .await
            .expect_err("privileged role must be refused");
        assert!(err.to_string().contains(role.as_str()));
        assert!(err.to_string().contains("ATOMO_RLS_ALLOW_BYPASS_ROLE"));
        atomo_server::rls::check_connection_role(&pool, true)
            .await
            .expect("escape hatch must allow with an ERROR log");
    } else {
        // Least-privilege role: fine under both settings.
        atomo_server::rls::check_connection_role(&pool, false)
            .await
            .expect("non-privileged role must pass");
        atomo_server::rls::check_connection_role(&pool, true)
            .await
            .expect("non-privileged role must pass");
    }
}
