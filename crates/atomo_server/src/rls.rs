//! Database tenant isolation. Server boot uses schema-aware required/global
//! policies through ensure_schema_rls. Legacy low-level policy helpers retain
//! their explicit permissive behavior for library callers. Application row
//! permissions are separately enforced by atomo::security; RLS is defense in depth.

use anyhow::Context;
use sqlx::PgPool;

/// The Postgres session variable the tenant-isolation policy reads.
pub const TENANT_SETTING: &str = "atomo.tenant_id";

/// The name of the generated per-table policy (stable so setup is idempotent).
pub const POLICY_NAME: &str = "atomo_tenant_isolation";

/// Returns whether RLS is enabled via the `ATOMO_ENABLE_RLS` env flag.
///
/// Accepts `true`/`1` (case-sensitive, matching the rest of the codebase's
/// env-flag convention). Default is **false**.
pub fn rls_enabled() -> bool {
    std::env::var("ATOMO_ENABLE_RLS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// Generate the idempotent DDL statements that enable RLS + (re)create the
/// tenant-isolation policy for a single table.
///
/// Split out (and pure) so it can be unit-tested without a database.
///
/// The policy is **permissive**: a row whose `tenant_id IS NULL` is always
/// visible/writable (single-tenant / unscoped data), and otherwise the row's
/// `tenant_id` must equal the `atomo.tenant_id` session var. `current_setting(…,
/// true)` returns NULL (the "missing_ok" form) when the var is unset, so a
/// request that never bound a tenant only sees NULL-tenant rows.
///
/// `DROP POLICY IF EXISTS` first makes re-running on boot safe even if the policy
/// definition changed between releases.
pub fn policy_statements_for(table: &str) -> Vec<String> {
    let qt = atomo::query::sql_builder::quote_ident(table);
    let qp = atomo::query::sql_builder::quote_ident(POLICY_NAME);
    vec![
        format!("ALTER TABLE {qt} ENABLE ROW LEVEL SECURITY;"),
        // FORCE so the table-owning role (which the app may connect as) is also
        // subject to the policy. Without this, RLS silently does nothing for the
        // owner — the common "it didn't work" footgun.
        format!("ALTER TABLE {qt} FORCE ROW LEVEL SECURITY;"),
        format!("DROP POLICY IF EXISTS {qp} ON {qt};"),
        format!(
            "CREATE POLICY {qp} ON {qt} \
             USING (tenant_id IS NULL OR tenant_id = current_setting('{TENANT_SETTING}', true)) \
             WITH CHECK (tenant_id IS NULL OR tenant_id = current_setting('{TENANT_SETTING}', true));"
        ),
    ]
}

/// Verify the connected database role cannot bypass RLS.
///
/// `FORCE ROW LEVEL SECURITY` covers the table *owner*, but Postgres superusers
/// and roles with `BYPASSRLS` skip policies unconditionally — with
/// `ATOMO_ENABLE_RLS` on, such a connection leaves tenant isolation silently
/// inert. Call at boot and fail fast rather than discover it in production.
///
/// `allow_bypass` (`ATOMO_RLS_ALLOW_BYPASS_ROLE`) downgrades the refusal to an
/// `ERROR` log for environments that knowingly run privileged connections
/// (e.g. a managed PG that only offers superuser). Escape hatches must be
/// deliberate, so the log stays loud.
pub async fn check_connection_role(pool: &PgPool, allow_bypass: bool) -> anyhow::Result<()> {
    let (role, rolsuper, rolbypassrls): (String, bool, bool) = sqlx::query_as(
        "SELECT rolname, rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user",
    )
    .fetch_one(pool)
    .await?;

    if !rolsuper && !rolbypassrls {
        return Ok(());
    }

    let why = match (rolsuper, rolbypassrls) {
        (true, true) => "SUPERUSER and BYPASSRLS",
        (true, false) => "SUPERUSER",
        (false, true) => "BYPASSRLS",
        _ => unreachable!(),
    };
    let msg = format!(
        "ATOMO_ENABLE_RLS is on but database role '{role}' has {why} — row-level security \
         is silently inert for this connection. Connect as a non-superuser, non-BYPASSRLS \
         role, or set ATOMO_RLS_ALLOW_BYPASS_ROLE=true to proceed anyway."
    );
    if allow_bypass {
        tracing::error!(role = %role, "{msg}");
        Ok(())
    } else {
        anyhow::bail!(msg)
    }
}

/// Enable RLS + (re)create the tenant-isolation policy on each given table.
///
/// - When `enabled` is **false** this is a **no-op** (returns `Ok(())`
///   immediately) — nothing is touched, behavior is unchanged. Callers pass
///   [`rls_enabled`].
/// - When **true**, runs [`policy_statements_for`] for every table in one
///   transaction. Idempotent and safe to run on every boot.
///
/// Any DDL failure (missing table/tenant_id, insufficient ownership, etc.) aborts
/// startup and rolls back the whole batch, including replaced policies and RLS
/// flags on earlier tables. The connection role must be able to alter every
/// table. The separate role-bypass escape hatch does not suppress these errors.
pub async fn ensure_rls_policies(
    pool: &PgPool,
    table_names: &[String],
    enabled: bool,
) -> anyhow::Result<()> {
    if !enabled {
        return Ok(());
    }

    // Startup callers may collect tables from a HashMap. Take DDL locks in a
    // stable order so concurrent instances do not lock the same tables backwards.
    let mut tables: Vec<_> = table_names.iter().collect();
    tables.sort_unstable();
    let mut tx = pool
        .begin()
        .await
        .context("begin RLS policy installation")?;
    for table in tables {
        for stmt in policy_statements_for(table) {
            // Dropping an uncommitted sqlx transaction rolls back every DDL
            // change; a failed replacement never leaves the old policy removed.
            sqlx::query(&stmt)
                .execute(&mut *tx)
                .await
                .with_context(|| {
                    format!("RLS policy installation failed for table '{table}'; refusing startup")
                })?;
        }
    }
    tx.commit()
        .await
        .context("commit RLS policy installation")?;

    tracing::info!(
        tables = table_names.len(),
        setting = TENANT_SETTING,
        "Row-Level Security enabled (ATOMO_ENABLE_RLS) — tenant isolation enforced by Postgres"
    );
    Ok(())
}

/// Bind the request's tenant to the `atomo.tenant_id` session var **for the
/// current transaction**, so the RLS policy resolves to this tenant.
///
/// `SET LOCAL` is **transaction-scoped**: it is reset at COMMIT/ROLLBACK and is
/// therefore safe under PgBouncer *transaction* pooling — the binding can never
/// leak onto another tenant's request that reuses the same physical connection.
/// For that guarantee to hold, the bind **and the queries it protects must run
/// inside the same transaction** on the same connection.
///
/// ## Usage (the required shape)
///
/// (`text` fence: an illustrative fragment, not a compilable doctest — the CI
/// `--ignored` pass would otherwise try to build it and fail.)
///
/// ```text
/// let mut tx = pool.begin().await?;
/// atomo_server::rls::bind_tenant(&mut tx, tenant_id.as_deref()).await?;
/// // ... all reads/writes for this request, on `&mut *tx` ...
/// tx.commit().await?;
/// ```
///
/// Passing `None` (no tenant on the request) leaves the var unset, so the policy
/// only exposes `tenant_id IS NULL` rows — the safe default.
///
/// `SET LOCAL` does not accept a bind parameter for its value, so the tenant is
/// applied via `set_config(name, value, is_local := true)`, which is the
/// parameter-safe equivalent and avoids any SQL-injection surface.
pub async fn bind_tenant(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Option<&str>,
) -> anyhow::Result<()> {
    match tenant_id {
        Some(tid) => {
            // set_config(setting, value, is_local=true) == SET LOCAL, but bindable.
            sqlx::query("SELECT set_config($1, $2, true)")
                .bind(TENANT_SETTING)
                .bind(tid)
                .execute(&mut **tx)
                .await?;
        }
        None => {
            // Nothing to bind; the policy falls back to NULL-tenant rows only.
        }
    }
    Ok(())
}

// =============================================================================
// PER-TRANSACTION WIRING SEAM (status: WIRED)
// =============================================================================
//
// The per-request binding is now wired through the `atomo` query executor:
//
//   1. `graphql_handler` (handlers.rs) wraps the whole request execution in
//      `atomo::graphql::with_tenant_scope(tenant, schema.execute(req))`, where
//      `tenant` is the same header value that gates `TenantCtx` (so app-layer
//      scoping and RLS binding can never disagree).
//   2. `with_tenant_scope` sets a task-local tenant for the request. When
//      `ATOMO_ENABLE_RLS` is on, `AtomoClient`'s read/write methods
//      (`find_many`/`find_unique`/`create`/`update_many`/`delete_many`/`count`/…)
//      run each statement inside `pool.begin()` + `SET LOCAL atomo.tenant_id`
//      (the `set_config(…, true)` form) on the same connection, then commit —
//      so the policy resolves to this tenant. The read cache is also tenant-keyed.
//   3. When RLS is off or no scope is active (SDK/projector/seed callers), the
//      methods run directly on the pool — byte-for-byte the prior path.
//
// Proven end-to-end against Postgres by:
//   - `atomo_server/tests/rls_enforcement.rs` (policy + `bind_tenant` primitive), and
//   - `atomo/tests/rls_executor.rs` (`find_many` under `with_tenant_scope`, incl. the
//     "forgot the WHERE" case and tenant-keyed cache).
//
// GraphQL WebSocket delivery applies verified tenant and row rules.
// Event-store per-tenant APIs remain a separate follow-up.

/// Schema-aware policies: existing null rows stay stored but are inaccessible in
/// required-tenant models. Only an explicit global model admits global rows.
/// Install the whole schema atomically, locking tables in stable name order.
/// An error restores all previous policies and flags and refuses startup.
pub async fn ensure_schema_rls(
    pool: &PgPool,
    schema: &atomo::schema::Schema,
) -> anyhow::Result<()> {
    let mut models: Vec<_> = schema
        .models
        .values()
        .filter(|m| m.fields.contains_key("id") && !m.fields.contains_key("_enum_type"))
        .map(|model| (atomo::query::sql_builder::table_name_for(model), model))
        .collect();
    models.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut tx = pool
        .begin()
        .await
        .context("begin schema RLS policy installation")?;
    for (table, model) in models {
        let mut statements = policy_statements_for(&table);
        let condition = if model.tenant_policy == atomo::schema::TenantPolicy::Global {
            "tenant_id IS NULL".to_owned()
        } else {
            format!(
                "tenant_id IS NOT NULL AND tenant_id = current_setting('{TENANT_SETTING}', true)"
            )
        };
        let qt = atomo::query::sql_builder::quote_ident(&table);
        let qp = atomo::query::sql_builder::quote_ident(POLICY_NAME);
        *statements.last_mut().unwrap() =
            format!("CREATE POLICY {qp} ON {qt} USING ({condition}) WITH CHECK ({condition})");
        for sql in statements {
            sqlx::query(&sql).execute(&mut *tx).await.with_context(|| {
                format!("RLS policy installation failed for table '{table}'; refusing startup")
            })?;
        }
    }
    tx.commit()
        .await
        .context("commit schema RLS policy installation")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_statements_are_idempotent_and_keyed_on_session_var() {
        let stmts = policy_statements_for("contacts");
        assert_eq!(stmts.len(), 4);
        assert!(stmts[0].contains("ENABLE ROW LEVEL SECURITY"));
        assert!(stmts[1].contains("FORCE ROW LEVEL SECURITY"));
        assert!(stmts[2].contains("DROP POLICY IF EXISTS"));
        let policy = &stmts[3];
        assert!(policy.contains(POLICY_NAME));
        assert!(policy.contains("tenant_id IS NULL"));
        assert!(policy.contains("current_setting('atomo.tenant_id', true)"));
        assert!(policy.contains("WITH CHECK"));
    }

    #[test]
    fn disabled_flag_is_default() {
        std::env::remove_var("ATOMO_ENABLE_RLS");
        assert!(!rls_enabled());
    }

    #[tokio::test]
    async fn disabled_installation_never_connects() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        ensure_rls_policies(&pool, &["missing_table".into()], false)
            .await
            .unwrap();
    }
}
