//! Lease-engine tests for the durable job store (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test jobs_store -- --ignored --test-threads=1
//! Covers the correctness-critical surface: idempotent enqueue, the lease→complete lifecycle,
//! stale-lease rejection, SKIP LOCKED concurrent dispatch, visibility-timeout reclaim, and the
//! retry→dead-letter policy.

use atomo_server::jobs::{FailOutcome, JobStore};
use serde_json::json;
use std::sync::Arc;

async fn store() -> JobStore {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let s = JobStore::new(pool, tx);
    s.init().await.unwrap();
    s
}

#[tokio::test]
#[ignore]
async fn enqueue_is_idempotent_on_key() {
    let s = store().await;
    let q = format!("idem-{}", uuid::Uuid::new_v4());
    let a = s
        .enqueue(&q, "k", json!({"n": 1}), Some("dupe"), 5, 0, None)
        .await
        .unwrap();
    let b = s
        .enqueue(&q, "k", json!({"n": 2}), Some("dupe"), 5, 0, None)
        .await
        .unwrap();
    assert_eq!(a, b, "same (queue,key) returns the same job id");
    // Distinct key → distinct job.
    let c = s
        .enqueue(&q, "k", json!({}), Some("other"), 5, 0, None)
        .await
        .unwrap();
    assert_ne!(a, c);
    // No key → always a new job (NULLs are distinct).
    let d = s
        .enqueue(&q, "k", json!({}), None, 5, 0, None)
        .await
        .unwrap();
    let e = s
        .enqueue(&q, "k", json!({}), None, 5, 0, None)
        .await
        .unwrap();
    assert_ne!(d, e);
}

#[tokio::test]
#[ignore]
async fn lease_complete_lifecycle_and_stale_lease_rejected() {
    let s = store().await;
    let q = format!("life-{}", uuid::Uuid::new_v4());
    let id = s
        .enqueue(&q, "k", json!({"v": 1}), None, 5, 0, None)
        .await
        .unwrap();
    assert_eq!(s.status(&id).await.unwrap().as_deref(), Some("queued"));

    let leased = s.lease(std::slice::from_ref(&q), 10, 30).await.unwrap();
    assert_eq!(leased.len(), 1);
    let job = &leased[0];
    assert_eq!(job.id, id);
    assert_eq!(job.attempts, 1, "lease increments attempts");
    assert_eq!(s.status(&id).await.unwrap().as_deref(), Some("leased"));

    // Heartbeat with the right lease extends; with a stale token is rejected.
    assert!(s.heartbeat(&id, &job.lease_id, 30).await.unwrap());
    assert!(!s.heartbeat(&id, "not-the-lease", 30).await.unwrap());

    // Complete with the lease succeeds; a second/stale complete is a no-op.
    assert!(s
        .complete(&id, &job.lease_id, json!({"ok": true}))
        .await
        .unwrap());
    assert_eq!(s.status(&id).await.unwrap().as_deref(), Some("succeeded"));
    assert!(!s.complete(&id, &job.lease_id, json!({})).await.unwrap());

    // A fresh lease finds nothing left in this queue.
    assert!(s.lease(&[q], 10, 30).await.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn skip_locked_gives_disjoint_leases_to_concurrent_workers() {
    let s = Arc::new(store().await);
    let q = format!("conc-{}", uuid::Uuid::new_v4());
    for _ in 0..6 {
        s.enqueue(&q, "k", json!({}), None, 5, 0, None)
            .await
            .unwrap();
    }
    // Two workers lease concurrently; together they must take all 6, none twice.
    let (s1, s2, q1, q2) = (s.clone(), s.clone(), q.clone(), q.clone());
    let (a, b) = tokio::join!(
        async move { s1.lease(&[q1], 3, 30).await.unwrap() },
        async move { s2.lease(&[q2], 3, 30).await.unwrap() },
    );
    let mut ids: Vec<String> = a.iter().chain(b.iter()).map(|j| j.id.clone()).collect();
    ids.sort();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "no job leased by two workers at once");
    assert_eq!(unique, 6, "all eligible jobs leased exactly once");
}

#[tokio::test]
#[ignore]
async fn expired_lease_is_reclaimed() {
    let s = store().await;
    let q = format!("reclaim-{}", uuid::Uuid::new_v4());
    let id = s
        .enqueue(&q, "k", json!({}), None, 5, 0, None)
        .await
        .unwrap();
    // Lease with a zero-second visibility window → immediately past once a moment elapses.
    let leased = s.lease(std::slice::from_ref(&q), 1, 0).await.unwrap();
    assert_eq!(leased.len(), 1);
    assert_eq!(s.status(&id).await.unwrap().as_deref(), Some("leased"));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let reclaimed = s.reclaim_expired().await.unwrap();
    assert!(reclaimed >= 1, "expired lease returned to the queue");
    assert_eq!(s.status(&id).await.unwrap().as_deref(), Some("queued"));
}

#[tokio::test]
#[ignore]
async fn fail_retries_then_dead_letters() {
    let s = store().await;

    // max_attempts=5: first failure retries with a backoff.
    let q = format!("retry-{}", uuid::Uuid::new_v4());
    let id = s
        .enqueue(&q, "k", json!({}), None, 5, 0, None)
        .await
        .unwrap();
    let job = s
        .lease(std::slice::from_ref(&q), 1, 30)
        .await
        .unwrap()
        .pop()
        .unwrap();
    let outcome = s.fail(&id, &job.lease_id, "boom", true).await.unwrap();
    assert_eq!(outcome, Some(FailOutcome::Retry { delay_secs: 5 }));
    assert_eq!(s.status(&id).await.unwrap().as_deref(), Some("queued"));
    // Failing with a stale lease now is a no-op.
    assert_eq!(
        s.fail(&id, &job.lease_id, "again", true).await.unwrap(),
        None
    );

    // max_attempts=1: the only attempt's failure dead-letters.
    let q2 = format!("dead-{}", uuid::Uuid::new_v4());
    let id2 = s
        .enqueue(&q2, "k", json!({}), None, 1, 0, None)
        .await
        .unwrap();
    let job2 = s.lease(&[q2], 1, 30).await.unwrap().pop().unwrap();
    let outcome2 = s.fail(&id2, &job2.lease_id, "nope", true).await.unwrap();
    assert_eq!(outcome2, Some(FailOutcome::DeadLetter));
    assert_eq!(s.status(&id2).await.unwrap().as_deref(), Some("dead"));

    // Non-retryable dead-letters immediately even with attempts to spare.
    let q3 = format!("nonretry-{}", uuid::Uuid::new_v4());
    let id3 = s
        .enqueue(&q3, "k", json!({}), None, 5, 0, None)
        .await
        .unwrap();
    let job3 = s.lease(&[q3], 1, 30).await.unwrap().pop().unwrap();
    let outcome3 = s.fail(&id3, &job3.lease_id, "fatal", false).await.unwrap();
    assert_eq!(outcome3, Some(FailOutcome::DeadLetter));
    assert_eq!(s.status(&id3).await.unwrap().as_deref(), Some("dead"));
}
