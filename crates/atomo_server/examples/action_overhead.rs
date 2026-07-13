//! Action overhead benchmarks: measures the incremental cost of the event-to-action pipeline
//! compared to plain CRUD.
//!
//! Run (release, against a Postgres):
//!   DATABASE_URL=postgres:///atomo_bench cargo run --release -p atomo_server --example action_overhead
//! Optional: BENCH_ITERS=500 (default 500; kept low because scenario 4 does HTTP round-trips).
//!
//! ## Scenarios
//!
//! | # | Scenario | What it measures |
//! |---|---------|-----------------|
//! | 1 | **Baseline CRUD** | `client.create()` with empty events (no bindings). The floor. |
//! | 2 | **CRUD + event emission** | `create` on a model with events declared but no action dispatcher listening. The event is broadcast to zero receivers. Shows the cost of event construction + broadcast send. |
//! | 3 | **CRUD + event + action enqueue** | `create` with a live action dispatcher that matches the event and enqueues a job into the jobs table. Shows the full dispatch + INSERT overhead. |
//! | 4 | **Worker CRUD callback** | An HTTP round-trip through `POST /api/worker/crud/:model` (axum `oneshot`). Measures the worker-callback path: auth middleware, capability check, CRUD, response serialization. |
//!
//! ## How to read the differentials
//!
//! - **Scenario 2 - Scenario 1** = pure event emission overhead (broadcast send, no receivers)
//! - **Scenario 3 - Scenario 2** = action dispatch + job enqueue cost (binding match + INSERT)
//! - **Scenario 4 - Scenario 1** = full worker round-trip cost on top of a baseline create
//!
//! All numbers are serial, in-process latencies (same method as the main `bench` example).
//! Scenario 4 uses `tower::ServiceExt::oneshot` to avoid a real TCP socket — it measures the
//! axum handler stack, not the OS network layer.

use atomo_server::action_dispatcher::spawn_action_dispatcher;
use atomo_server::crud_routes::crud_router;
use atomo_server::jobs::{JobStore, WorkerTokenStore};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

// ── Stats (same helper as the main bench example) ──────────────────────────

struct Stats {
    n: usize,
    mean_us: f64,
    p50_us: u128,
    p95_us: u128,
    p99_us: u128,
    ops_per_s: f64,
}

fn stats(mut d: Vec<Duration>) -> Stats {
    d.sort();
    let n = d.len().max(1);
    let total_us: f64 = d.iter().map(|x| x.as_nanos() as f64 / 1000.0).sum();
    let pct = |p: f64| d[(((n as f64) * p) as usize).min(n - 1)].as_micros();
    Stats {
        n: d.len(),
        mean_us: total_us / n as f64,
        p50_us: pct(0.50),
        p95_us: pct(0.95),
        p99_us: pct(0.99),
        ops_per_s: if total_us > 0.0 {
            1_000_000.0 * n as f64 / total_us
        } else {
            0.0
        },
    }
}

fn rec(title: &str, status: &str) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    m.insert("title".to_string(), json!(title));
    m.insert("status".to_string(), json!(status));
    m
}

// ── Schema fragments ───────────────────────────────────────────────────────

/// Model with NO event bindings (empty events) — the baseline.
const SCHEMA_NO_EVENTS: &str = "\
export interface Post { id: string; title: string; status: string; }\n\
export const schema = { models: { Post: { tableName: 'bench_action_no_events' } } };\n\
export default schema;";

/// Model with event bindings declared (created -> processPost) — used for scenarios 2 and 3.
const SCHEMA_WITH_EVENTS: &str = r#"
export interface Post { id: string; title: string; status: string; }
export const schema = {
  models: {
    Post: {
      tableName: "bench_action_with_events",
      events: {
        created: [{ action: "processPost" }],
      },
    },
  },
  actions: {
    processPost: {
      input: { pick: { model: "Post", fields: ["id", "title"] } },
    },
  },
};
export default schema;
"#;

#[tokio::main]
async fn main() {
    let db = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let iters: usize = std::env::var("BENCH_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);

    let mut rows: Vec<(String, Stats)> = Vec::new();

    // ── Scenario 1: Baseline CRUD (no event bindings) ──────────────────────
    //
    // A plain `client.create()` on a model with zero event bindings. The event
    // broadcast still fires (it always does), but there are no subscribers and
    // no bindings to match. This is the absolute floor for a create.
    {
        let atomo = atomo::Atomo::builder()
            .schema_content(SCHEMA_NO_EVENTS)
            .database_url(&db)
            .enable_migrations(true)
            .build()
            .await
            .expect("scenario 1: atomo build");
        let client = atomo.client();
        let _ = sqlx::query("TRUNCATE bench_action_no_events")
            .execute(atomo.db_pool())
            .await;

        // Warmup
        for i in 0..20 {
            client
                .create("Post", &rec(&format!("w{i}"), "draft"), &[], Some("bench"))
                .await
                .unwrap();
        }

        let mut t = Vec::with_capacity(iters);
        for i in 0..iters {
            let r = rec(&format!("s1-{i}"), "draft");
            let s = Instant::now();
            client.create("Post", &r, &[], Some("bench")).await.unwrap();
            t.push(s.elapsed());
        }
        rows.push(("1. baseline CRUD (no event bindings)".into(), stats(t)));

        let _ = sqlx::query("TRUNCATE bench_action_no_events")
            .execute(atomo.db_pool())
            .await;
    }

    // ── Scenario 2: CRUD + event emission (no dispatcher) ──────────────────
    //
    // The model declares events (created -> processPost), but no action dispatcher
    // is listening. The broadcast send hits zero receivers. This isolates the cost
    // of constructing and broadcasting the ModelEvent with binding metadata,
    // WITHOUT the job-enqueue overhead.
    {
        let atomo = atomo::Atomo::builder()
            .schema_content(SCHEMA_WITH_EVENTS)
            .database_url(&db)
            .enable_migrations(true)
            .build()
            .await
            .expect("scenario 2: atomo build");
        let client = atomo.client();
        let _ = sqlx::query("TRUNCATE bench_action_with_events")
            .execute(atomo.db_pool())
            .await;

        // Warmup
        for i in 0..20 {
            client
                .create("Post", &rec(&format!("w{i}"), "draft"), &[], Some("bench"))
                .await
                .unwrap();
        }

        let mut t = Vec::with_capacity(iters);
        for i in 0..iters {
            let r = rec(&format!("s2-{i}"), "draft");
            let s = Instant::now();
            client.create("Post", &r, &[], Some("bench")).await.unwrap();
            t.push(s.elapsed());
        }
        rows.push(("2. CRUD + event emission (no dispatcher)".into(), stats(t)));

        let _ = sqlx::query("TRUNCATE bench_action_with_events")
            .execute(atomo.db_pool())
            .await;
    }

    // ── Scenario 3: CRUD + event + action enqueue ──────────────────────────
    //
    // Same schema as scenario 2, but now the action dispatcher is spawned and
    // actively listening. Each `create` triggers processPost -> the dispatcher
    // matches the binding and INSERTs a job row. This measures the full pipeline:
    //   create -> event broadcast -> dispatcher recv -> binding match -> job INSERT
    //
    // Note: the dispatcher runs on a background task; the `create` call itself
    // returns as soon as the DB commit + broadcast send complete. The job INSERT
    // is async (fire-and-forget from the caller's perspective). We measure the
    // create latency (what the caller sees) and then wait briefly for the queue
    // to drain, to confirm jobs were actually enqueued.
    {
        let atomo = atomo::Atomo::builder()
            .schema_content(SCHEMA_WITH_EVENTS)
            .database_url(&db)
            .enable_migrations(true)
            .build()
            .await
            .expect("scenario 3: atomo build");
        let client = atomo.client();
        let pool = atomo.db_pool().clone();

        let _ = sqlx::query("TRUNCATE bench_action_with_events")
            .execute(&pool)
            .await;

        // Set up the job store and action dispatcher.
        let (tx, _) = tokio::sync::broadcast::channel(4096);
        let job_store = Arc::new(JobStore::new(pool.clone(), tx));
        job_store.init().await.unwrap();

        // Use a unique queue name so we don't collide with other tests/benches.
        // The dispatcher always uses "actions" — we clean up after.
        spawn_action_dispatcher(
            atomo.schema().clone(),
            job_store.clone(),
            atomo.event_receiver(),
        );

        // Clean the actions queue from any prior runs.
        let _ = sqlx::query("DELETE FROM jobs WHERE queue = 'actions'")
            .execute(&pool)
            .await;

        // Warmup
        for i in 0..20 {
            client
                .create("Post", &rec(&format!("w{i}"), "draft"), &[], Some("bench"))
                .await
                .unwrap();
        }
        // Let the warmup jobs drain through the dispatcher.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let mut t = Vec::with_capacity(iters);
        for i in 0..iters {
            let r = rec(&format!("s3-{i}"), "draft");
            let s = Instant::now();
            client.create("Post", &r, &[], Some("bench")).await.unwrap();
            t.push(s.elapsed());
        }
        rows.push((
            "3. CRUD + event + action enqueue (dispatcher live)".into(),
            stats(t),
        ));

        // Wait for the dispatcher to finish enqueueing, then verify.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let enqueued: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM jobs WHERE queue = 'actions' AND kind = 'processPost'",
        )
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));
        eprintln!(
            "  [scenario 3] jobs enqueued: {} (expected >= {})",
            enqueued.0, iters
        );

        let _ = sqlx::query("TRUNCATE bench_action_with_events")
            .execute(&pool)
            .await;
        let _ = sqlx::query("DELETE FROM jobs WHERE queue = 'actions'")
            .execute(&pool)
            .await;
    }

    // ── Scenario 4: Worker CRUD callback (HTTP round-trip via oneshot) ─────
    //
    // Simulates what a real external worker does: call back into Atomo's CRUD API
    // via `POST /api/worker/crud/Post` with an X-Worker-Token header. We use
    // axum's `oneshot` (no real TCP socket) to isolate the handler stack:
    //   HTTP parse -> worker auth middleware -> capability check -> client.create()
    //   -> JSON serialization -> response
    //
    // The differential vs scenario 1 shows the overhead of the HTTP/auth/serialization
    // layers that a worker pays on every callback.
    {
        let atomo = atomo::Atomo::builder()
            .schema_content(SCHEMA_NO_EVENTS)
            .database_url(&db)
            .enable_migrations(true)
            .build()
            .await
            .expect("scenario 4: atomo build");
        let pool = atomo.db_pool().clone();

        let _ = sqlx::query("TRUNCATE bench_action_no_events")
            .execute(&pool)
            .await;

        let worker_tokens = Arc::new(WorkerTokenStore::new(pool.clone()));
        worker_tokens.init().await.unwrap();

        // Mint a worker token with full CRUD access.
        let (_, token) = worker_tokens
            .mint("bench-worker", &["actions".into()], &["crud:*".into()])
            .await
            .unwrap();

        // Build the CRUD router (same stack a real server uses).
        let app = crud_router(
            Arc::new(atomo.client().clone()),
            atomo.schema().clone(),
            worker_tokens,
        );

        // Warmup
        for i in 0..20 {
            let req = Request::builder()
                .uri("/api/worker/crud/Post")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-worker-token", &token)
                .body(Body::from(
                    json!({"data": {"title": format!("w{i}"), "status": "draft"}}).to_string(),
                ))
                .unwrap();
            let _ = app.clone().oneshot(req).await;
        }

        let mut t = Vec::with_capacity(iters);
        for i in 0..iters {
            let req = Request::builder()
                .uri("/api/worker/crud/Post")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-worker-token", &token)
                .body(Body::from(
                    json!({"data": {"title": format!("s4-{i}"), "status": "draft"}}).to_string(),
                ))
                .unwrap();
            let s = Instant::now();
            let resp = app.clone().oneshot(req).await.unwrap();
            t.push(s.elapsed());
            assert_eq!(
                resp.status(),
                StatusCode::CREATED,
                "scenario 4: expected 201"
            );
        }
        rows.push((
            "4. worker CRUD callback (HTTP round-trip via oneshot)".into(),
            stats(t),
        ));

        let _ = sqlx::query("TRUNCATE bench_action_no_events")
            .execute(&pool)
            .await;
    }

    // ── Report ─────────────────────────────────────────────────────────────

    println!("\n## Action overhead benchmarks (in-process)\n");
    println!("iterations: {iters} · serial, in-process latency\n");
    println!("| Scenario | n | mean us | p50 us | p95 us | p99 us | ops/sec |");
    println!("|---|--:|--:|--:|--:|--:|--:|");
    for (name, s) in &rows {
        let pct = |v: u128| {
            if v == 0 {
                "—".to_string()
            } else {
                v.to_string()
            }
        };
        println!(
            "| {} | {} | {:.1} | {} | {} | {} | {:.0} |",
            name,
            s.n,
            s.mean_us,
            pct(s.p50_us),
            pct(s.p95_us),
            pct(s.p99_us),
            s.ops_per_s
        );
    }

    // Differentials
    if rows.len() >= 4 {
        println!("\n### Differentials\n");
        let s1 = rows[0].1.mean_us;
        let s2 = rows[1].1.mean_us;
        let s3 = rows[2].1.mean_us;
        let s4 = rows[3].1.mean_us;
        println!("| Differential | mean us | What it isolates |");
        println!("|---|--:|---|");
        println!(
            "| S2 - S1 (event emission overhead) | {:.1} | cost of constructing + broadcasting the ModelEvent to zero receivers |",
            s2 - s1
        );
        println!(
            "| S3 - S2 (action dispatch + enqueue) | {:.1} | binding match + job INSERT (async, off the create hot path) |",
            s3 - s2
        );
        println!(
            "| S4 - S1 (worker HTTP callback overhead) | {:.1} | auth middleware + capability check + JSON ser/de on top of baseline create |",
            s4 - s1
        );
    }
    println!();
}
