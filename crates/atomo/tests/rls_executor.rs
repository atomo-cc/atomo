//! Phase-3 RLS through the actual query executor: proves `AtomoClient::find_many` run inside
//! `with_tenant_scope` (with `ATOMO_ENABLE_RLS=1`) returns only the bound tenant's rows + the
//! NULL-tenant rows — i.e. the executor wiring (request task-local scope → per-statement
//! `SET LOCAL atomo.tenant_id`) enforces RLS, not just the raw `bind_tenant` primitive.
//!
//! Also exercises the tenant-keyed read cache: the same query for tenant A then tenant B must
//! not serve A's cached rows to B.
//!
//! Requires Postgres via DATABASE_URL. Seeds/cleans a `widgets` table.
//! Run: cargo test -p atomo --test rls_executor -- --ignored

use atomo::client::{with_tenant_scope, AtomoClient};

fn schema_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rls_widget_schema.ts")
        .to_string_lossy()
        .into_owned()
}

async fn widget_names(c: &AtomoClient, tenant: Option<&str>) -> Vec<String> {
    let t = tenant.map(|s| s.to_string());
    let rows = with_tenant_scope(t, c.find_many("Widget", &[], &[], None, None, &[]))
        .await
        .unwrap();
    let mut names: Vec<String> = rows
        .iter()
        .filter_map(|r| r.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();
    names.sort();
    names
}

#[tokio::test]
#[ignore]
async fn find_many_enforces_rls_under_tenant_scope() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");

    let atomo = atomo::Atomo::builder()
        .schema_file(schema_path())
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .expect("schema should build + migrate (creates `widgets` with tenant_id)");
    let c = atomo.client();
    let pool = c.db_pool();

    // Superuser/BYPASSRLS roles bypass RLS — can't demonstrate enforcement; skip cleanly.
    let is_super: bool = sqlx::query_scalar("SELECT current_setting('is_superuser') = 'on'")
        .fetch_one(pool)
        .await
        .unwrap();
    if is_super {
        eprintln!("SKIP rls_executor: superuser role bypasses RLS");
        return;
    }

    // Seed BEFORE enabling RLS (afterwards WITH CHECK blocks unbound inserts of tenant rows).
    sqlx::query("DELETE FROM widgets").execute(pool).await.ok();
    sqlx::query(
        "INSERT INTO widgets (id, tenant_id, name) VALUES \
         ('a1','tenant-a','A1'), ('b1','tenant-b','B1'), ('g1', NULL, 'GLOBAL') \
         ON CONFLICT (id) DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();

    // Enable RLS on the model table (inline DDL mirrors atomo_server::rls::policy_statements_for).
    for stmt in [
        "ALTER TABLE widgets ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE widgets FORCE ROW LEVEL SECURITY",
        "DROP POLICY IF EXISTS atomo_tenant_isolation ON widgets",
        "CREATE POLICY atomo_tenant_isolation ON widgets \
         USING (tenant_id IS NULL OR tenant_id = current_setting('atomo.tenant_id', true)) \
         WITH CHECK (tenant_id IS NULL OR tenant_id = current_setting('atomo.tenant_id', true))",
    ] {
        sqlx::query(stmt).execute(pool).await.unwrap();
    }

    std::env::set_var("ATOMO_ENABLE_RLS", "1");

    // The executor (find_many) must enforce isolation purely from the task-local scope —
    // note: NO tenant_id WHERE clause is passed, so this is the "forgot the WHERE" case that
    // only RLS protects.
    assert_eq!(
        widget_names(c, Some("tenant-a")).await,
        vec!["A1".to_string(), "GLOBAL".to_string()],
        "tenant-a via find_many must see only A + global"
    );
    assert_eq!(
        widget_names(c, Some("tenant-b")).await,
        vec!["B1".to_string(), "GLOBAL".to_string()],
        "tenant-b via find_many must see only B + global (not A's cached rows)"
    );
    assert_eq!(
        widget_names(c, None).await,
        vec!["GLOBAL".to_string()],
        "unbound find_many must see only NULL-tenant rows"
    );

    // Cleanup: disable RLS + remove rows (leave the table; harmless).
    std::env::remove_var("ATOMO_ENABLE_RLS");
    for stmt in [
        "ALTER TABLE widgets NO FORCE ROW LEVEL SECURITY",
        "ALTER TABLE widgets DISABLE ROW LEVEL SECURITY",
        "DROP POLICY IF EXISTS atomo_tenant_isolation ON widgets",
        "DELETE FROM widgets",
    ] {
        sqlx::query(stmt).execute(pool).await.ok();
    }
}
