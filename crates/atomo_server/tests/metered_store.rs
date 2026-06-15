//! Tests for the generic metered-command primitives (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test metered_store -- --ignored --test-threads=1
//!
//! Covers the correctness-critical surface: single-use token consumption, windowed budget
//! reservation, **concurrent reservations never over-committing a budget** (advisory-lock
//! serialization), and **transactional atomicity** — a rolled-back caller transaction consumes no
//! token, reserves no budget, and enqueues no job, while a committed one does all three.

use atomo_server::jobs::JobStore;
use atomo_server::metered::{BudgetLedger, ExpiringTokenStore};
use serde_json::json;

async fn pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    sqlx::PgPool::connect(&url).await.unwrap()
}

async fn tokens() -> ExpiringTokenStore {
    let s = ExpiringTokenStore::new(pool().await);
    s.init().await.unwrap();
    s
}

async fn ledger() -> BudgetLedger {
    let s = BudgetLedger::new(pool().await);
    s.init().await.unwrap();
    s
}

async fn jobs() -> JobStore {
    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let s = JobStore::new(pool().await, tx);
    s.init().await.unwrap();
    s
}

// ---- ExpiringTokenStore -------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn token_is_single_use_and_payload_round_trips() {
    let store = tokens().await;
    let subject = format!("sess-{}", uuid::Uuid::new_v4());
    let token = store
        .mint("upload", &subject, json!({ "assetId": "a1" }), 3600)
        .await
        .unwrap();
    assert!(token.starts_with("tok_"));

    // First consume wins and returns the attached payload.
    let mut tx = pool().await.begin().await.unwrap();
    let consumed = store
        .consume(&mut tx, "upload", &subject, &token)
        .await
        .unwrap()
        .expect("token consumes once");
    assert_eq!(consumed.payload, json!({ "assetId": "a1" }));
    tx.commit().await.unwrap();

    // Second consume is rejected — single use.
    let mut tx = pool().await.begin().await.unwrap();
    assert!(store
        .consume(&mut tx, "upload", &subject, &token)
        .await
        .unwrap()
        .is_none());
    tx.commit().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn token_rejects_wrong_scope_and_expired() {
    let store = tokens().await;
    let subject = format!("sess-{}", uuid::Uuid::new_v4());
    let token = store
        .mint("upload", &subject, json!({}), 3600)
        .await
        .unwrap();

    let mut tx = pool().await.begin().await.unwrap();
    // Wrong purpose, wrong subject → no match (and does not consume).
    assert!(store
        .consume(&mut tx, "other", &subject, &token)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .consume(&mut tx, "upload", "someone-else", &token)
        .await
        .unwrap()
        .is_none());
    tx.commit().await.unwrap();

    // An already-expired token (negative ttl) never consumes.
    let expired = store.mint("upload", &subject, json!({}), -1).await.unwrap();
    let mut tx = pool().await.begin().await.unwrap();
    assert!(store
        .consume(&mut tx, "upload", &subject, &expired)
        .await
        .unwrap()
        .is_none());
    tx.commit().await.unwrap();

    // The original token is still unconsumed after the rejected attempts.
    let mut tx = pool().await.begin().await.unwrap();
    assert!(store
        .consume(&mut tx, "upload", &subject, &token)
        .await
        .unwrap()
        .is_some());
    tx.commit().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn token_consume_rolls_back_with_transaction() {
    let store = tokens().await;
    let subject = format!("sess-{}", uuid::Uuid::new_v4());
    let token = store
        .mint("upload", &subject, json!({}), 3600)
        .await
        .unwrap();

    // Consume inside a transaction, then roll back.
    let mut tx = pool().await.begin().await.unwrap();
    assert!(store
        .consume(&mut tx, "upload", &subject, &token)
        .await
        .unwrap()
        .is_some());
    tx.rollback().await.unwrap();

    // The rollback undid the consume — the token is still usable exactly once.
    let mut tx = pool().await.begin().await.unwrap();
    assert!(store
        .consume(&mut tx, "upload", &subject, &token)
        .await
        .unwrap()
        .is_some());
    tx.commit().await.unwrap();
}

// ---- BudgetLedger -------------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn budget_reserve_enforces_windowed_limit() {
    let store = ledger().await;
    let scope = format!("global-{}", uuid::Uuid::new_v4());

    // Two reservations of 100 fit under a 250 limit; the third does not.
    for _ in 0..2 {
        let mut tx = pool().await.begin().await.unwrap();
        assert!(store
            .try_reserve(&mut tx, &scope, 100, 250, 3600)
            .await
            .unwrap());
        tx.commit().await.unwrap();
    }
    let mut tx = pool().await.begin().await.unwrap();
    assert!(!store
        .try_reserve(&mut tx, &scope, 100, 250, 3600)
        .await
        .unwrap());
    tx.commit().await.unwrap();

    assert_eq!(store.windowed_sum(&scope, 3600).await.unwrap(), 200);

    // Non-positive amounts/limits never reserve.
    let mut tx = pool().await.begin().await.unwrap();
    assert!(!store
        .try_reserve(&mut tx, &scope, 0, 250, 3600)
        .await
        .unwrap());
    assert!(!store
        .try_reserve(&mut tx, &scope, 50, 0, 3600)
        .await
        .unwrap());
    tx.commit().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn budget_reserve_rolls_back_with_transaction() {
    let store = ledger().await;
    let scope = format!("global-{}", uuid::Uuid::new_v4());

    let mut tx = pool().await.begin().await.unwrap();
    assert!(store
        .try_reserve(&mut tx, &scope, 100, 250, 3600)
        .await
        .unwrap());
    tx.rollback().await.unwrap();

    // The rolled-back reservation does not count against the budget.
    assert_eq!(store.windowed_sum(&scope, 3600).await.unwrap(), 0);
}

#[tokio::test]
#[ignore]
async fn concurrent_reserves_never_overcommit() {
    let p = pool().await;
    let scope = format!("global-{}", uuid::Uuid::new_v4());

    // 8 racing reservations of 100 against a 250 budget: the advisory lock must let exactly 2 win.
    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = p.clone();
        let scope = scope.clone();
        handles.push(tokio::spawn(async move {
            let store = BudgetLedger::new(pool.clone());
            let mut tx = pool.begin().await.unwrap();
            let ok = store
                .try_reserve(&mut tx, &scope, 100, 250, 3600)
                .await
                .unwrap();
            if ok {
                tx.commit().await.unwrap();
            } else {
                tx.rollback().await.unwrap();
            }
            ok
        }));
    }
    let mut wins = 0;
    for h in handles {
        if h.await.unwrap() {
            wins += 1;
        }
    }
    assert_eq!(wins, 2, "exactly two reservations fit under the budget");

    let store = BudgetLedger::new(p.clone());
    assert_eq!(store.windowed_sum(&scope, 3600).await.unwrap(), 200);
}

// ---- enqueue_tx + the combined atomic command --------------------------------------------------

#[tokio::test]
#[ignore]
async fn enqueue_tx_is_transactional_and_idempotent() {
    let store = jobs().await;
    let queue = format!("q-{}", uuid::Uuid::new_v4());

    // Rolled-back enqueue leaves no job.
    let mut tx = pool().await.begin().await.unwrap();
    let (rolled, created) = store
        .enqueue_tx(&mut tx, &queue, "k", json!({}), Some("idem"), 5, 0, None)
        .await
        .unwrap();
    assert!(created);
    tx.rollback().await.unwrap();
    assert!(store.status(&rolled).await.unwrap().is_none());

    // Committed enqueue is visible; a second enqueue on the same (queue, key) is idempotent.
    let mut tx = pool().await.begin().await.unwrap();
    let (id, created) = store
        .enqueue_tx(&mut tx, &queue, "k", json!({}), Some("idem"), 5, 0, None)
        .await
        .unwrap();
    assert!(created);
    tx.commit().await.unwrap();
    assert_eq!(store.status(&id).await.unwrap().as_deref(), Some("queued"));

    let mut tx = pool().await.begin().await.unwrap();
    let (again, created) = store
        .enqueue_tx(&mut tx, &queue, "k", json!({}), Some("idem"), 5, 0, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(again, id, "same (queue, key) returns the existing id");
    assert!(!created, "no duplicate created");
}

#[tokio::test]
#[ignore]
async fn metered_command_is_all_or_nothing() {
    let p = pool().await;
    let token_store = tokens().await;
    let budget = ledger().await;
    let job_store = jobs().await;

    let subject = format!("sess-{}", uuid::Uuid::new_v4());
    let scope = format!("global-{}", uuid::Uuid::new_v4());
    let queue = format!("q-{}", uuid::Uuid::new_v4());
    let token = token_store
        .mint("upload", &subject, json!({ "assetId": "a1" }), 3600)
        .await
        .unwrap();

    // Attempt 1: reserve + consume + enqueue, then a simulated downstream failure rolls everything
    // back as one unit.
    let mut tx = p.begin().await.unwrap();
    assert!(budget
        .try_reserve(&mut tx, &scope, 100, 1000, 3600)
        .await
        .unwrap());
    assert!(token_store
        .consume(&mut tx, "upload", &subject, &token)
        .await
        .unwrap()
        .is_some());
    let (failed_id, _) = job_store
        .enqueue_tx(
            &mut tx,
            &queue,
            "demo",
            json!({}),
            Some("run-1"),
            5,
            0,
            None,
        )
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    // Nothing persisted: no budget spent, token still usable, no job.
    assert_eq!(budget.windowed_sum(&scope, 3600).await.unwrap(), 0);
    assert!(job_store.status(&failed_id).await.unwrap().is_none());

    // Attempt 2: the same command succeeds and commits — all three effects persist together.
    let mut tx = p.begin().await.unwrap();
    assert!(budget
        .try_reserve(&mut tx, &scope, 100, 1000, 3600)
        .await
        .unwrap());
    let consumed = token_store
        .consume(&mut tx, "upload", &subject, &token)
        .await
        .unwrap()
        .expect("token still usable after the rollback");
    assert_eq!(consumed.payload, json!({ "assetId": "a1" }));
    let (ok_id, created) = job_store
        .enqueue_tx(
            &mut tx,
            &queue,
            "demo",
            json!({}),
            Some("run-1"),
            5,
            0,
            None,
        )
        .await
        .unwrap();
    assert!(created);
    tx.commit().await.unwrap();
    job_store.emit_enqueued(&ok_id, &queue, "demo");

    assert_eq!(budget.windowed_sum(&scope, 3600).await.unwrap(), 100);
    assert_eq!(
        job_store.status(&ok_id).await.unwrap().as_deref(),
        Some("queued")
    );
    // The token is now spent for good.
    let mut tx = p.begin().await.unwrap();
    assert!(token_store
        .consume(&mut tx, "upload", &subject, &token)
        .await
        .unwrap()
        .is_none());
    tx.commit().await.unwrap();
}
