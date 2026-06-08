//! Durable, event-sourced job queue + lease engine for **trusted external workers**.
//!
//! This is the brain side of the external-worker model (see
//! `docs/guide/advanced/workers-and-blobs-design.md`): a side-effect-heavy workload (calling
//! external APIs, browser automation, media pipelines) runs in an out-of-process worker that
//! **pulls** durable jobs from here, does the messy I/O, and reports results back. Atomo owns the
//! event-sourced lifecycle; the mess lives in the worker.
//!
//! Delivery is **at-least-once**: a lease has a visibility timeout, and an expired lease returns the
//! job to the queue (crash recovery). Workers therefore must tolerate a job running twice — the
//! `idempotency_key` makes *enqueue* idempotent, and content-addressed media neutralizes duplicate
//! side-effects. Dispatch uses `SELECT … FOR UPDATE SKIP LOCKED` so many workers can lease
//! concurrently from one Postgres without lock contention.
//!
//! This module is the **lease engine** (the correctness-critical core). The HTTP lease API and
//! worker-token auth layer on top of it in a later slice; nothing here is network-exposed yet.

use anyhow::Result;
use atomo::events::{EventType, ModelEvent};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tokio::sync::broadcast;

const DEFAULT_BASE_BACKOFF_SECS: u64 = 5;
const DEFAULT_CAP_BACKOFF_SECS: u64 = 3600;

/// What to do with a job after a failed attempt — decided purely from the attempt counters so it
/// is unit-testable without a database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailOutcome {
    /// Re-enqueue after `delay_secs` (backoff).
    Retry { delay_secs: u64 },
    /// Attempts exhausted (or non-retryable) — park in the dead-letter state.
    DeadLetter,
}

/// Exponential backoff (base · 2^(attempts-1)) capped at `cap_secs`, no jitter so it is
/// deterministic for tests. `attempts` is 1-based: the number of the attempt that just failed.
pub fn backoff_secs(attempts: u32, base_secs: u64, cap_secs: u64) -> u64 {
    if attempts <= 1 {
        return base_secs.min(cap_secs);
    }
    // Cap the shift so `1 << shift` can never overflow u64.
    let shift = (attempts - 1).min(20);
    base_secs.saturating_mul(1u64 << shift).min(cap_secs)
}

/// Decide a failed attempt's fate. Non-retryable errors, or having reached `max_attempts`, go
/// straight to the dead-letter state; otherwise retry after a backoff.
pub fn decide_on_fail(
    attempts: u32,
    max_attempts: u32,
    retryable: bool,
    base_secs: u64,
    cap_secs: u64,
) -> FailOutcome {
    if !retryable || attempts >= max_attempts {
        FailOutcome::DeadLetter
    } else {
        FailOutcome::Retry {
            delay_secs: backoff_secs(attempts, base_secs, cap_secs),
        }
    }
}

/// A job handed to a worker by `lease`. `lease_id` is the per-job token the worker must echo back to
/// `heartbeat`/`complete`/`fail` (a stale token — e.g. after the lease expired and the job was
/// re-leased — is rejected, so a zombie worker can't complete someone else's job).
#[derive(Debug, Clone)]
pub struct LeasedJob {
    pub id: String,
    pub queue: String,
    pub kind: String,
    pub payload: Value,
    pub attempts: i32,
    pub max_attempts: i32,
    pub lease_id: String,
}

/// Durable job store over Postgres. The `jobs` table is the queue working set; lifecycle
/// transitions are also emitted as `Job` model events (audit/history/projections), so the job log
/// is event-sourced like everything else in Atomo.
pub struct JobStore {
    pool: PgPool,
    sender: broadcast::Sender<ModelEvent>,
    base_backoff_secs: u64,
    cap_backoff_secs: u64,
}

impl JobStore {
    pub fn new(pool: PgPool, sender: broadcast::Sender<ModelEvent>) -> Self {
        Self {
            pool,
            sender,
            base_backoff_secs: DEFAULT_BASE_BACKOFF_SECS,
            cap_backoff_secs: DEFAULT_CAP_BACKOFF_SECS,
        }
    }

    /// Idempotent schema. Statements run individually — a prepared query cannot carry multiple
    /// commands (the registry hit exactly this bug at boot).
    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS jobs (
                id              TEXT PRIMARY KEY,
                queue           TEXT NOT NULL,
                kind            TEXT NOT NULL,
                status          TEXT NOT NULL,
                payload         JSONB NOT NULL DEFAULT '{}'::jsonb,
                result          JSONB,
                error           TEXT,
                idempotency_key TEXT,
                attempts        INT NOT NULL DEFAULT 0,
                max_attempts    INT NOT NULL DEFAULT 5,
                lease_id        TEXT,
                worker_id       TEXT,
                visible_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                tenant_id       TEXT,
                priority        INT NOT NULL DEFAULT 0,
                created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        // Idempotent enqueue: at most one live row per (queue, idempotency_key) when a key is given.
        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS jobs_idempotency
             ON jobs (queue, idempotency_key) WHERE idempotency_key IS NOT NULL",
        )
        .execute(&self.pool)
        .await?;
        // Dispatch path: cheap lookup of the next eligible job per queue.
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS jobs_dispatch
             ON jobs (queue, status, priority DESC, visible_at)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Enqueue a job. Idempotent when `idempotency_key` is set: a second enqueue for the same
    /// `(queue, key)` returns the existing id and does **not** emit a duplicate event.
    #[allow(clippy::too_many_arguments)]
    pub async fn enqueue(
        &self,
        queue: &str,
        kind: &str,
        payload: Value,
        idempotency_key: Option<&str>,
        max_attempts: i32,
        priority: i32,
        tenant_id: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let inserted: Option<String> = sqlx::query(
            "INSERT INTO jobs (id, queue, kind, status, payload, idempotency_key, max_attempts, priority, tenant_id)
             VALUES ($1, $2, $3, 'queued', $4, $5, $6, $7, $8)
             ON CONFLICT (queue, idempotency_key) WHERE idempotency_key IS NOT NULL
             DO NOTHING
             RETURNING id",
        )
        .bind(&id)
        .bind(queue)
        .bind(kind)
        .bind(&payload)
        .bind(idempotency_key)
        .bind(max_attempts)
        .bind(priority)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|r| r.get::<String, _>("id"));

        match inserted {
            Some(new_id) => {
                let mut extra = HashMap::new();
                extra.insert("queue".to_string(), json!(queue));
                extra.insert("kind".to_string(), json!(kind));
                extra.insert("status".to_string(), json!("queued"));
                self.emit(EventType::Created, &new_id, extra);
                Ok(new_id)
            }
            // Conflict: a job with this (queue, key) already exists — return its id.
            None => {
                let existing: String =
                    sqlx::query("SELECT id FROM jobs WHERE queue = $1 AND idempotency_key = $2")
                        .bind(queue)
                        .bind(idempotency_key)
                        .fetch_one(&self.pool)
                        .await?
                        .get("id");
                Ok(existing)
            }
        }
    }

    /// Atomically lease up to `capacity` eligible jobs from any of `queues`, giving each a fresh
    /// per-job `lease_id` and a `visible_at` deadline `visibility_secs` in the future. `SKIP LOCKED`
    /// lets concurrent workers lease disjoint sets without blocking. Increments `attempts`.
    pub async fn lease(
        &self,
        queues: &[String],
        capacity: i64,
        visibility_secs: i64,
    ) -> Result<Vec<LeasedJob>> {
        let rows = sqlx::query(
            "WITH picked AS (
                 SELECT id FROM jobs
                 WHERE queue = ANY($1) AND status = 'queued' AND visible_at <= NOW()
                 ORDER BY priority DESC, visible_at
                 FOR UPDATE SKIP LOCKED
                 LIMIT $2
             )
             UPDATE jobs j
                SET status = 'leased',
                    lease_id = gen_random_uuid()::text,
                    attempts = j.attempts + 1,
                    visible_at = NOW() + ($3 || ' seconds')::interval,
                    updated_at = NOW()
               FROM picked
              WHERE j.id = picked.id
              RETURNING j.id, j.queue, j.kind, j.payload, j.attempts, j.max_attempts, j.lease_id",
        )
        .bind(queues)
        .bind(capacity)
        .bind(visibility_secs.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut leased = Vec::with_capacity(rows.len());
        for r in rows {
            let job = LeasedJob {
                id: r.get("id"),
                queue: r.get("queue"),
                kind: r.get("kind"),
                payload: r.get("payload"),
                attempts: r.get("attempts"),
                max_attempts: r.get("max_attempts"),
                lease_id: r.get("lease_id"),
            };
            let mut extra = HashMap::new();
            extra.insert("status".to_string(), json!("leased"));
            extra.insert("attempts".to_string(), json!(job.attempts));
            self.emit(EventType::Custom, &job.id, extra);
            leased.push(job);
        }
        Ok(leased)
    }

    /// Extend a held lease (worker still alive, long handler). Rejected if the lease token is stale
    /// or the job is no longer leased. Returns whether the lease was extended.
    pub async fn heartbeat(&self, id: &str, lease_id: &str, visibility_secs: i64) -> Result<bool> {
        let res = sqlx::query(
            "UPDATE jobs SET visible_at = NOW() + ($3 || ' seconds')::interval, updated_at = NOW()
             WHERE id = $1 AND lease_id = $2 AND status = 'leased'",
        )
        .bind(id)
        .bind(lease_id)
        .bind(visibility_secs.to_string())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Mark a leased job succeeded with its result. Rejected (returns false) if the lease token is
    /// stale or the job already left the leased state — so a zombie worker can't clobber a result.
    pub async fn complete(&self, id: &str, lease_id: &str, result: Value) -> Result<bool> {
        let res = sqlx::query(
            "UPDATE jobs SET status = 'succeeded', result = $3, lease_id = NULL, updated_at = NOW()
             WHERE id = $1 AND lease_id = $2 AND status = 'leased'",
        )
        .bind(id)
        .bind(lease_id)
        .bind(&result)
        .execute(&self.pool)
        .await?;
        let ok = res.rows_affected() > 0;
        if ok {
            let mut extra = HashMap::new();
            extra.insert("status".to_string(), json!("succeeded"));
            self.emit(EventType::Custom, id, extra);
        }
        Ok(ok)
    }

    /// Report a failed attempt. The retry policy (attempt counters + `retryable`) decides whether
    /// the job is re-enqueued after a backoff or dead-lettered. Returns `None` if the lease token is
    /// stale / the job isn't leased (no-op), else the outcome taken.
    pub async fn fail(
        &self,
        id: &str,
        lease_id: &str,
        error: &str,
        retryable: bool,
    ) -> Result<Option<FailOutcome>> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT attempts, max_attempts FROM jobs
             WHERE id = $1 AND lease_id = $2 AND status = 'leased' FOR UPDATE",
        )
        .bind(id)
        .bind(lease_id)
        .fetch_optional(&mut *tx)
        .await?;
        let row = match row {
            Some(r) => r,
            None => {
                tx.rollback().await?;
                return Ok(None);
            }
        };
        let attempts: i32 = row.get("attempts");
        let max_attempts: i32 = row.get("max_attempts");
        let outcome = decide_on_fail(
            attempts.max(0) as u32,
            max_attempts.max(0) as u32,
            retryable,
            self.base_backoff_secs,
            self.cap_backoff_secs,
        );

        match &outcome {
            FailOutcome::Retry { delay_secs } => {
                sqlx::query(
                    "UPDATE jobs SET status = 'queued', lease_id = NULL, error = $2,
                            visible_at = NOW() + ($3 || ' seconds')::interval, updated_at = NOW()
                     WHERE id = $1",
                )
                .bind(id)
                .bind(error)
                .bind((*delay_secs as i64).to_string())
                .execute(&mut *tx)
                .await?;
            }
            FailOutcome::DeadLetter => {
                sqlx::query(
                    "UPDATE jobs SET status = 'dead', lease_id = NULL, error = $2, updated_at = NOW()
                     WHERE id = $1",
                )
                .bind(id)
                .bind(error)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;

        let mut extra = HashMap::new();
        extra.insert("error".to_string(), json!(error));
        extra.insert(
            "status".to_string(),
            json!(match outcome {
                FailOutcome::Retry { .. } => "queued",
                FailOutcome::DeadLetter => "dead",
            }),
        );
        self.emit(EventType::Custom, id, extra);
        Ok(Some(outcome))
    }

    /// Return jobs whose lease expired (worker crashed/stalled) to the queue. Run periodically.
    /// Returns the number reclaimed.
    pub async fn reclaim_expired(&self) -> Result<u64> {
        let res = sqlx::query(
            "UPDATE jobs SET status = 'queued', lease_id = NULL, worker_id = NULL, updated_at = NOW()
             WHERE status = 'leased' AND visible_at < NOW()",
        )
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    /// Current status of a job (`queued`/`leased`/`succeeded`/`dead`), or None if unknown.
    pub async fn status(&self, id: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT status FROM jobs WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("status")))
    }

    fn emit(&self, event_type: EventType, id: &str, extra: HashMap<String, Value>) {
        let mut data = extra;
        data.insert("id".to_string(), json!(id));
        let _ = self.sender.send(ModelEvent {
            event_type,
            model_name: "Job".to_string(),
            data,
            previous_data: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_id: uuid::Uuid::new_v4().to_string(),
            actor: None,
        });
    }
}

/// A verified worker principal. `queues` is its capability set: which queues it may lease from
/// (`["*"]` = any). A worker is trusted *relative to the sandbox* but still least-privilege —
/// the token scopes what it can pull.
#[derive(Debug, Clone)]
pub struct WorkerIdentity {
    pub id: String,
    pub name: String,
    pub queues: Vec<String>,
}

impl WorkerIdentity {
    /// Whether this worker may lease from `queue`.
    pub fn may_lease(&self, queue: &str) -> bool {
        self.queues.iter().any(|q| q == "*" || q == queue)
    }
}

/// Issues and verifies **worker tokens** — a credential class distinct from user JWTs. The
/// plaintext token is shown once at mint; only its SHA-256 is stored, so a DB read can't recover a
/// usable token. Verification is a single indexed lookup by hash.
pub struct WorkerTokenStore {
    pool: PgPool,
}

impl WorkerTokenStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS worker_tokens (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                token_sha256 TEXT NOT NULL UNIQUE,
                queues       TEXT[] NOT NULL DEFAULT '{}',
                is_revoked   BOOLEAN NOT NULL DEFAULT false,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mint a new worker token scoped to `queues` (use `["*"]` for any). Returns
    /// `(id, plaintext_token)` — the plaintext is **not recoverable later**, only its hash is stored.
    pub async fn mint(&self, name: &str, queues: &[String]) -> Result<(String, String)> {
        let id = uuid::Uuid::new_v4().to_string();
        // High-entropy secret (~244 bits across two v4 UUIDs); prefix marks it as a worker token.
        let token = format!(
            "wkr_{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        sqlx::query(
            "INSERT INTO worker_tokens (id, name, token_sha256, queues) VALUES ($1, $2, $3, $4)",
        )
        .bind(&id)
        .bind(name)
        .bind(sha256_hex(&token))
        .bind(queues)
        .execute(&self.pool)
        .await?;
        Ok((id, token))
    }

    /// Verify a presented token; returns the worker identity (with its allowed queues) or None.
    pub async fn verify(&self, token: &str) -> Result<Option<WorkerIdentity>> {
        let row = sqlx::query(
            "SELECT id, name, queues FROM worker_tokens
             WHERE token_sha256 = $1 AND is_revoked = false",
        )
        .bind(sha256_hex(token))
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| WorkerIdentity {
            id: r.get("id"),
            name: r.get("name"),
            queues: r.get("queues"),
        }))
    }

    /// Revoke a token by id. Returns whether a live token was revoked.
    pub async fn revoke(&self, id: &str) -> Result<bool> {
        let res = sqlx::query(
            "UPDATE worker_tokens SET is_revoked = true WHERE id = $1 AND is_revoked = false",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }
}

/// Lowercase hex SHA-256 of `s` (used to store/look up worker tokens without keeping the plaintext).
fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(s.as_bytes());
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{backoff_secs, decide_on_fail, sha256_hex, FailOutcome, WorkerIdentity};

    #[test]
    fn worker_capability_scoping() {
        let w = WorkerIdentity {
            id: "1".into(),
            name: "media".into(),
            queues: vec!["media-gen".into()],
        };
        assert!(w.may_lease("media-gen"));
        assert!(!w.may_lease("billing"));
        let star = WorkerIdentity {
            id: "2".into(),
            name: "any".into(),
            queues: vec!["*".into()],
        };
        assert!(star.may_lease("anything"));
    }

    #[test]
    fn sha256_hex_is_stable_64_hex() {
        let h = sha256_hex("wkr_abc");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h, sha256_hex("wkr_abc"));
        assert_ne!(h, sha256_hex("wkr_abd"));
    }

    #[test]
    fn backoff_is_exponential_and_capped() {
        // base 5, cap 3600
        assert_eq!(backoff_secs(1, 5, 3600), 5); // first failure → base
        assert_eq!(backoff_secs(2, 5, 3600), 10);
        assert_eq!(backoff_secs(3, 5, 3600), 20);
        assert_eq!(backoff_secs(4, 5, 3600), 40);
        // Capped.
        assert_eq!(backoff_secs(100, 5, 3600), 3600);
        // No overflow at absurd attempt counts.
        assert_eq!(backoff_secs(1_000_000, 5, 3600), 3600);
        // base==cap stays put.
        assert_eq!(backoff_secs(1, 30, 30), 30);
    }

    #[test]
    fn fail_decision_matrix() {
        // Retryable, attempts remaining → retry with backoff.
        assert_eq!(
            decide_on_fail(1, 5, true, 5, 3600),
            FailOutcome::Retry { delay_secs: 5 }
        );
        assert_eq!(
            decide_on_fail(3, 5, true, 5, 3600),
            FailOutcome::Retry { delay_secs: 20 }
        );
        // Last attempt reached → dead-letter.
        assert_eq!(decide_on_fail(5, 5, true, 5, 3600), FailOutcome::DeadLetter);
        assert_eq!(decide_on_fail(6, 5, true, 5, 3600), FailOutcome::DeadLetter);
        // Non-retryable → dead-letter immediately, regardless of attempts left.
        assert_eq!(
            decide_on_fail(1, 5, false, 5, 3600),
            FailOutcome::DeadLetter
        );
    }
}
