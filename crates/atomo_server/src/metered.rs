//! Generic primitives for **metered, single-use, transactionally-composed commands**.
//!
//! These are platform building blocks for any consumer that exposes a *metered* command — a
//! credit/billing ledger, a metered-API gateway, a rate-limited public command — and needs the
//! whole command to succeed or fail as one unit. They own no business policy: scopes, purposes,
//! limits, and windows are all caller-supplied opaque values. Nothing here knows what is being
//! metered.
//!
//! Two stores, both designed to compose **inside a caller's transaction** so a metered command can
//! reserve budget, consume a one-time token, and enqueue a [`crate::jobs`] job atomically (see
//! [`crate::jobs::JobStore::enqueue_tx`]):
//!
//! - [`ExpiringTokenStore`] — mint an opaque single-use token, store only its SHA-256, and consume
//!   it exactly once before it expires. The plaintext is shown once at mint and is never
//!   recoverable from the database.
//! - [`BudgetLedger`] — an append-only integer-unit ledger with a windowed-sum capacity check.
//!   [`BudgetLedger::try_reserve`] is serialized per scope with a transaction-scoped advisory lock,
//!   so concurrent reservations on the same scope can never over-commit a budget.
//!
//! Both consume/reserve operations take `&mut PgConnection` and **must be called inside a
//! transaction**: single-use and budget guarantees rely on the caller committing (or rolling back)
//! the surrounding work. Read-only helpers never mutate state.

use anyhow::Result;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

/// Lowercase hex SHA-256 of `s`. Tokens are stored and looked up only by this digest, so a database
/// read can never recover a usable token.
fn sha256_hex(s: &str) -> String {
    let digest = Sha256::digest(s.as_bytes());
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Derive a stable 64-bit advisory-lock key from an opaque scope string (first 8 bytes of its
/// SHA-256, big-endian). Used to serialize budget reservations per scope without a schema lock.
fn advisory_key(scope: &str) -> i64 {
    let digest = Sha256::digest(scope.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    i64::from_be_bytes(bytes)
}

/// Issues and consumes **single-use, expiring opaque tokens**. Only the SHA-256 of each token is
/// stored; the plaintext is returned once at [`mint`](ExpiringTokenStore::mint) and is not
/// recoverable later. A token is bound to a `purpose` (a namespace, e.g. `"upload"`) and an opaque
/// `subject` (e.g. a session identifier the caller chooses); consumption requires both to match and
/// succeeds at most once before `expires_at`.
pub struct ExpiringTokenStore {
    pool: PgPool,
}

/// A token successfully consumed from an [`ExpiringTokenStore`] — the opaque `payload` the minter
/// attached, returned to the caller exactly once.
#[derive(Debug, Clone)]
pub struct ConsumedToken {
    pub payload: Value,
}

impl ExpiringTokenStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent schema. Statements run individually — a prepared query cannot carry multiple
    /// commands.
    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS expiring_tokens (
                token_sha256 TEXT PRIMARY KEY,
                purpose      TEXT NOT NULL,
                subject      TEXT NOT NULL,
                payload      JSONB NOT NULL DEFAULT '{}'::jsonb,
                expires_at   TIMESTAMPTZ NOT NULL,
                consumed_at  TIMESTAMPTZ,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        // Housekeeping lookup: find expired/consumed rows to purge.
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS expiring_tokens_expiry ON expiring_tokens (expires_at)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mint a token for `purpose` + `subject` valid for `ttl_secs`, attaching an opaque `payload`
    /// the caller will receive back on consume. Returns the **plaintext token** (shown once; only
    /// its hash is stored). Generates the token from two UUIDs like the worker-token mint path.
    pub async fn mint(
        &self,
        purpose: &str,
        subject: &str,
        payload: Value,
        ttl_secs: i64,
    ) -> Result<String> {
        let token = format!(
            "tok_{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        sqlx::query(
            "INSERT INTO expiring_tokens (token_sha256, purpose, subject, payload, expires_at)
             VALUES ($1, $2, $3, $4, NOW() + ($5 || ' seconds')::interval)",
        )
        .bind(sha256_hex(&token))
        .bind(purpose)
        .bind(subject)
        .bind(&payload)
        .bind(ttl_secs.to_string())
        .execute(&self.pool)
        .await?;
        Ok(token)
    }

    /// Atomically consume `token` for `purpose` + `subject` **inside the caller's transaction**.
    /// Succeeds at most once: a single `UPDATE ... WHERE consumed_at IS NULL ... RETURNING` row lock
    /// makes concurrent consumers race for the one row, so only one wins. Returns the attached
    /// payload, or `None` if the token is unknown, already consumed, expired, or the
    /// `purpose`/`subject` do not match. The consume is durable only when the caller commits.
    pub async fn consume(
        &self,
        conn: &mut sqlx::PgConnection,
        purpose: &str,
        subject: &str,
        token: &str,
    ) -> Result<Option<ConsumedToken>> {
        let row = sqlx::query(
            "UPDATE expiring_tokens
                SET consumed_at = NOW()
              WHERE token_sha256 = $1 AND purpose = $2 AND subject = $3
                AND consumed_at IS NULL AND expires_at > NOW()
              RETURNING payload",
        )
        .bind(sha256_hex(token))
        .bind(purpose)
        .bind(subject)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| ConsumedToken {
            payload: r.get::<Value, _>("payload"),
        }))
    }

    /// Delete tokens that expired before now (read path is unaffected; this only reclaims rows).
    /// Returns the number removed. Wire to a scheduler under the caller's retention policy.
    pub async fn purge_expired(&self) -> Result<u64> {
        let res = sqlx::query("DELETE FROM expiring_tokens WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}

/// An append-only **integer-unit budget ledger**. Each row is a signed `amount` (e.g. micro-USD or
/// credits — the unit is the caller's) charged against an opaque `scope` (e.g. `"global:hourly"` or
/// a per-tenant key the caller composes). Capacity is the windowed sum over a recent interval; a
/// reservation is just a positive entry that keeps the windowed sum within `limit`.
pub struct BudgetLedger {
    pool: PgPool,
}

impl BudgetLedger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent schema. Statements run individually.
    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS budget_ledger (
                id         TEXT PRIMARY KEY,
                scope      TEXT NOT NULL,
                amount     BIGINT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        // Windowed-sum path: sum recent entries per scope.
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS budget_ledger_window ON budget_ledger (scope, created_at)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Sum of entries for `scope` within the last `window_secs` (read-only, no locking). Useful for
    /// reporting and circuit-breaker checks; reservations must use [`try_reserve`](Self::try_reserve)
    /// for the atomic path.
    pub async fn windowed_sum(&self, scope: &str, window_secs: i64) -> Result<i64> {
        let row = sqlx::query(
            "SELECT COALESCE(SUM(amount), 0)::bigint AS total FROM budget_ledger
              WHERE scope = $1 AND created_at > NOW() - ($2 || ' seconds')::interval",
        )
        .bind(scope)
        .bind(window_secs.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<i64, _>("total"))
    }

    /// Atomically reserve `amount` against `scope` **inside the caller's transaction**: take a
    /// transaction-scoped advisory lock keyed by `scope`, recompute the windowed sum, and insert the
    /// reservation only if `windowed_sum + amount <= limit`. The advisory lock serializes concurrent
    /// reservations on the same scope (released on commit/rollback), so two callers can never both
    /// pass a check that only one budget allows. Returns whether the reservation was made.
    ///
    /// `amount` and `limit` must be positive; a non-positive `amount` reserves nothing and returns
    /// `false` (a metered command always charges something).
    pub async fn try_reserve(
        &self,
        conn: &mut sqlx::PgConnection,
        scope: &str,
        amount: i64,
        limit: i64,
        window_secs: i64,
    ) -> Result<bool> {
        if amount <= 0 || limit <= 0 {
            return Ok(false);
        }
        // Per-scope serialization for the read-then-insert. Transaction-scoped: auto-released at
        // commit/rollback, so a caller cannot leak the lock.
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(advisory_key(scope))
            .execute(&mut *conn)
            .await?;
        let used: i64 = sqlx::query(
            "SELECT COALESCE(SUM(amount), 0)::bigint AS total FROM budget_ledger
              WHERE scope = $1 AND created_at > NOW() - ($2 || ' seconds')::interval",
        )
        .bind(scope)
        .bind(window_secs.to_string())
        .fetch_one(&mut *conn)
        .await?
        .get::<i64, _>("total");
        if used.saturating_add(amount) > limit {
            return Ok(false);
        }
        sqlx::query("INSERT INTO budget_ledger (id, scope, amount) VALUES ($1, $2, $3)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(scope)
            .bind(amount)
            .execute(&mut *conn)
            .await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::{advisory_key, sha256_hex};

    #[test]
    fn token_hash_is_stable_64_hex_and_not_plaintext() {
        let h = sha256_hex("tok_secret");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h, sha256_hex("tok_secret"));
        assert_ne!(h, "tok_secret");
        assert_ne!(h, sha256_hex("tok_secres"));
    }

    #[test]
    fn advisory_key_is_deterministic_and_scope_specific() {
        assert_eq!(advisory_key("global:hourly"), advisory_key("global:hourly"));
        assert_ne!(advisory_key("global:hourly"), advisory_key("global:daily"));
    }
}
