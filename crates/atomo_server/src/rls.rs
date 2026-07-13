//! Opt-in, DB-enforced multi-tenant **Row-Level Security (RLS)**.
//!
//! This is the *defense-in-depth* half of Atomo's multi-tenant story (the
//! "pool" model: shared schema + `tenant_id` + Postgres RLS). The application
//! layer already scopes reads/writes by `tenant_id` (see `auth.rs` +
//! `handlers.rs`); RLS makes a **forgotten `WHERE tenant_id = …` clause unable
//! to leak across tenants** because the database itself filters every row.
//!
//! # Opt-in / additive
//!
//! Everything here is gated behind the `ATOMO_ENABLE_RLS` env flag (default
//! **false**). When the flag is off, [`ensure_rls_policies`] is a no-op and
//! behavior is byte-for-byte unchanged. Turning it on is additive: the policy
//! `tenant_id IS NULL OR tenant_id = current_setting('atomo.tenant_id', true)`
//! is permissive for `NULL` tenants, so single-tenant deployments (which leave
//! `tenant_id` NULL) keep working untouched.
//!
//! # The four pieces (see docs/guide/advanced/multi-project-design.md)
//!
//! 1. **Policy generation + setup** — [`ensure_rls_policies`] runs, per model
//!    table: `ENABLE ROW LEVEL SECURITY` + a `CREATE POLICY` keyed on the
//!    `atomo.tenant_id` session var (with `WITH CHECK` for writes). Idempotent.
//! 2. **Per-transaction binding** — [`bind_tenant`] runs `SET LOCAL
//!    atomo.tenant_id = $1` inside a transaction, so the policy resolves to the
//!    request's tenant. `SET LOCAL` is transaction-scoped → safe under PgBouncer
//!    transaction pooling.
//! 3. **Boot hook** — called from `server.rs run()` after tables exist.
//! 4. **Event-store scoping** — out of scope here (planned follow-up).
//!
//! # Caveat: table owners / superusers bypass RLS
//!
//! By default the table *owner* and superusers bypass RLS. For full enforcement
//! the application should connect as a non-owner role, or the tables should be
//! created with `FORCE ROW LEVEL SECURITY`. We emit `FORCE` as well so the
//! owning role is also constrained — see [`ensure_rls_policies`].

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
    vec![
        format!("ALTER TABLE {table} ENABLE ROW LEVEL SECURITY;"),
        // FORCE so the table-owning role (which the app may connect as) is also
        // subject to the policy. Without this, RLS silently does nothing for the
        // owner — the common "it didn't work" footgun.
        format!("ALTER TABLE {table} FORCE ROW LEVEL SECURITY;"),
        format!("DROP POLICY IF EXISTS {POLICY_NAME} ON {table};"),
        format!(
            "CREATE POLICY {POLICY_NAME} ON {table} \
             USING (tenant_id IS NULL OR tenant_id = current_setting('{TENANT_SETTING}', true)) \
             WITH CHECK (tenant_id IS NULL OR tenant_id = current_setting('{TENANT_SETTING}', true));"
        ),
    ]
}

/// Enable RLS + (re)create the tenant-isolation policy on each given table.
///
/// - When `enabled` is **false** this is a **no-op** (returns `Ok(())`
///   immediately) — nothing is touched, behavior is unchanged. Callers pass
///   [`rls_enabled`].
/// - When **true**, runs [`policy_statements_for`] for every table. Idempotent
///   and safe to run on every boot.
///
/// A table that does not exist (or has no `tenant_id` column) produces a warning
/// rather than aborting boot — RLS is opt-in defense-in-depth and must not take
/// the server down. (In practice every model table has a nullable `tenant_id`;
/// see `generate_migrations` in the `atomo` crate.)
pub async fn ensure_rls_policies(
    pool: &PgPool,
    table_names: &[String],
    enabled: bool,
) -> anyhow::Result<()> {
    if !enabled {
        return Ok(());
    }

    let mut applied = 0usize;
    for table in table_names {
        let mut ok = true;
        for stmt in policy_statements_for(table) {
            if let Err(e) = sqlx::query(&stmt).execute(pool).await {
                tracing::warn!(
                    table = %table,
                    error = %e,
                    statement = %stmt,
                    "RLS setup failed for table — skipping (server continues without RLS on it)"
                );
                ok = false;
                break;
            }
        }
        if ok {
            applied += 1;
        }
    }

    tracing::info!(
        tables = applied,
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
// Remaining follow-ups: the GraphQL *subscription/WS* path is not yet scoped, and
// event-store per-tenant scoping (step 4) is still planned.

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
}
