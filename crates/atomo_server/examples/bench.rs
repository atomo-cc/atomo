//! Atomo engine micro-benchmarks. Run (release, against a Postgres):
//!   DATABASE_URL=postgres:///atomo_bench cargo run --release -p atomo_server --example bench
//! Optional: BENCH_ITERS=5000 (default 2000).
//!
//! **Honest scope:** these are **engine-level, in-process** latencies (data layer, job lease
//! engine, plugin hook path) — they isolate component cost and deliberately exclude HTTP framing,
//! the network, and GraphQL resolution. They answer "what does each core operation cost," not
//! "requests/sec through the full stack." See `docs/guide/advanced/benchmarks.md` for method +
//! recorded results on a stated machine.

use atomo::hooks::{HookContext, HookRunner};
use atomo::query::{WhereClause, WhereOperator};
use atomo_server::jobs::JobStore;
use atomo_server::wasm_hooks::WasmHookRunner;
use atomo_server::wasm_plugins::WasmPluginManager;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

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

fn rec(title: &str) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    m.insert("title".to_string(), json!(title));
    m
}

fn make_ctx() -> HookContext {
    let mut data = HashMap::new();
    data.insert("email".to_string(), json!("a@b.com"));
    data.insert("tags".to_string(), json!(["x"]));
    HookContext {
        model_name: "Contact".to_string(),
        operation: "create".to_string(),
        data,
        user_id: None,
    }
}

#[tokio::main]
async fn main() {
    let db = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let iters: usize = std::env::var("BENCH_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    let mut rows: Vec<(String, Stats)> = Vec::new();

    // ----- Data layer: create + find_many -----
    let atomo = atomo::Atomo::builder()
        .schema_content(
            "export interface Note { id: string; title: string; }\n\
             export const schema = { models: { Note: { tableName: 'bench_notes' } } };\n\
             export default schema;",
        )
        .database_url(&db)
        .enable_migrations(true)
        .build()
        .await
        .expect("atomo build");
    let client = atomo.client();
    let _ = sqlx::query("TRUNCATE bench_notes")
        .execute(atomo.db_pool())
        .await;

    for i in 0..50 {
        client
            .create("Note", &rec(&format!("warm{i}")), &[], Some("bench"))
            .await
            .unwrap();
    }
    let mut t = Vec::with_capacity(iters);
    for i in 0..iters {
        let r = rec(&format!("n{i}"));
        let s = Instant::now();
        client.create("Note", &r, &[], Some("bench")).await.unwrap();
        t.push(s.elapsed());
    }
    rows.push(("data layer: create (insert + event)".to_string(), stats(t)));

    // create_many: a 100-row batch commits in ONE transaction (one fsync for the batch). Reported
    // as per-row latency so it compares directly to single `create` above.
    let batch_size = 100usize;
    let batches = (iters / batch_size).max(1);
    let mut t = Vec::with_capacity(batches);
    for b in 0..batches {
        let batch: Vec<_> = (0..batch_size).map(|i| rec(&format!("b{b}-{i}"))).collect();
        let s = Instant::now();
        client
            .create_many("Note", &batch, Some("bench"))
            .await
            .unwrap();
        t.push(s.elapsed() / batch_size as u32); // per-row
    }
    rows.push((
        format!("data layer: create_many (per row, batch={batch_size})"),
        stats(t),
    ));

    // update_many: update a single row by id, repeatedly (per-call latency, like create — now the
    // UPDATE + its event commit in one transaction).
    let id_of =
        |r: &HashMap<String, Value>| r.get("id").and_then(|v| v.as_str()).unwrap().to_string();
    let by_id = |id: &str| {
        vec![WhereClause {
            field: "id".to_string(),
            operator: WhereOperator::Equals,
            value: json!(id),
        }]
    };
    let seed_id = id_of(
        &client
            .create("Note", &rec("seed"), &[], Some("bench"))
            .await
            .unwrap(),
    );
    let mut_iters = iters.min(500);
    let mut t = Vec::with_capacity(mut_iters);
    for i in 0..mut_iters {
        let patch = rec(&format!("u{i}"));
        let s = Instant::now();
        client
            .update_many("Note", &by_id(&seed_id), &patch, &[], Some("bench"))
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "data layer: update_many (1 row by id)".to_string(),
        stats(t),
    ));

    // delete_many: soft-delete a single row by id (row created untimed, the delete is timed).
    let mut t = Vec::with_capacity(mut_iters);
    for i in 0..mut_iters {
        let id = id_of(
            &client
                .create("Note", &rec(&format!("d{i}")), &[], Some("bench"))
                .await
                .unwrap(),
        );
        let s = Instant::now();
        client
            .delete_many("Note", &by_id(&id), Some("bench"))
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "data layer: delete_many (1 row by id)".to_string(),
        stats(t),
    ));

    // BULK update_many / delete_many: one call matching ~500 rows (one UPDATE + chunked event
    // INSERT), reported per affected row to show the amortized bulk cost. A few rounds for stats.
    let by_title = |t: &str| {
        vec![WhereClause {
            field: "title".to_string(),
            operator: WhereOperator::Equals,
            value: json!(t),
        }]
    };
    let bulk_n = 500usize;
    let mut tu = Vec::new();
    let mut td = Vec::new();
    for r in 0..5 {
        // update round
        let tag = format!("bulku-{r}");
        let batch: Vec<_> = (0..bulk_n).map(|_| rec(&tag)).collect();
        client
            .create_many("Note", &batch, Some("bench"))
            .await
            .unwrap();
        let patch = rec(&format!("bulku-done-{r}"));
        let s = Instant::now();
        let n = client
            .update_many("Note", &by_title(&tag), &patch, &[], Some("bench"))
            .await
            .unwrap()
            .len();
        tu.push(s.elapsed() / n.max(1) as u32);
        // delete round
        let dtag = format!("bulkd-{r}");
        let batch: Vec<_> = (0..bulk_n).map(|_| rec(&dtag)).collect();
        client
            .create_many("Note", &batch, Some("bench"))
            .await
            .unwrap();
        let s = Instant::now();
        let n = client
            .delete_many("Note", &by_title(&dtag), Some("bench"))
            .await
            .unwrap();
        td.push(s.elapsed() / n.max(1) as u32);
    }
    rows.push((
        format!("data layer: update_many (per row, ~{bulk_n} matched)"),
        stats(tu),
    ));
    rows.push((
        format!("data layer: delete_many (per row, ~{bulk_n} matched)"),
        stats(td),
    ));

    let mut t = Vec::with_capacity(iters);
    for _ in 0..iters {
        let s = Instant::now();
        client
            .find_many("Note", &[], &[], Some(20), Some(0), &[])
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "data layer: find_many hot (limit 20, cache hit)".to_string(),
        stats(t),
    ));

    // find_many cold: a write between each read invalidates the model cache (strong mode),
    // so every read is a DB round trip — the true cold-read cost.
    let read_iters = iters.min(500);
    let mut t = Vec::with_capacity(read_iters);
    for i in 0..read_iters {
        client
            .create("Note", &rec(&format!("cold{i}")), &[], Some("bench"))
            .await
            .unwrap();
        let s = Instant::now();
        client
            .find_many("Note", &[], &[], Some(20), Some(0), &[])
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "data layer: find_many cold (limit 20, cache miss)".to_string(),
        stats(t),
    ));

    // ----- find_unique cold / hot -----
    let target_id = id_of(
        &client
            .create("Note", &rec("unique-target"), &[], Some("bench"))
            .await
            .unwrap(),
    );
    let by_target = by_id(&target_id);

    // Cold: write invalidates cache before each read
    let mut t = Vec::with_capacity(read_iters);
    for i in 0..read_iters {
        client
            .create("Note", &rec(&format!("ucold{i}")), &[], Some("bench"))
            .await
            .unwrap();
        let s = Instant::now();
        client
            .find_unique("Note", &by_target, &[])
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "data layer: find_unique cold (cache miss)".to_string(),
        stats(t),
    ));

    // Hot: same record repeatedly (cache hit after first)
    client.find_unique("Note", &by_target, &[]).await.unwrap();
    let mut t = Vec::with_capacity(iters);
    for _ in 0..iters {
        let s = Instant::now();
        client
            .find_unique("Note", &by_target, &[])
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "data layer: find_unique hot (cache hit)".to_string(),
        stats(t),
    ));

    // ----- count hot (cache hit after first call) -----
    client.count("Note", &[]).await.unwrap(); // warm
    let mut t = Vec::with_capacity(iters);
    for _ in 0..iters {
        let s = Instant::now();
        client.count("Note", &[]).await.unwrap();
        t.push(s.elapsed());
    }
    rows.push(("data layer: count hot (cache hit)".to_string(), stats(t)));

    // count cold: write invalidates cache before each count
    let mut t = Vec::with_capacity(read_iters);
    for i in 0..read_iters {
        client
            .create("Note", &rec(&format!("cnt{i}")), &[], Some("bench"))
            .await
            .unwrap();
        let s = Instant::now();
        client.count("Note", &[]).await.unwrap();
        t.push(s.elapsed());
    }
    rows.push(("data layer: count cold (cache miss)".to_string(), stats(t)));

    // ----- Relation: find_many with include (hasMany) -----
    let rel_schema = "\
        export interface RelNote { id: string; title: string; }\n\
        export interface RelTag { id: string; label: string; relNoteId: string; }\n\
        export const schema = { models: {\
            RelNote: { tableName: 'bench_rel_notes',\
                relationships: { tags: { type: 'hasMany', model: 'RelTag', foreignKey: 'relNoteId' } } },\
            RelTag: { tableName: 'bench_rel_tags',\
                relationships: { note: { type: 'belongsTo', model: 'RelNote', foreignKey: 'relNoteId' } } }\
        } };\n\
        export default schema;";
    let rel_atomo = atomo::Atomo::builder()
        .schema_content(rel_schema)
        .database_url(&db)
        .enable_migrations(true)
        .build()
        .await
        .expect("rel atomo build");
    let rel_client = rel_atomo.client();
    let _ = sqlx::query("TRUNCATE bench_rel_notes, bench_rel_tags")
        .execute(rel_atomo.db_pool())
        .await;
    // Seed: 20 notes, each with 3 tags
    for i in 0..20 {
        let note = rel_client
            .create("RelNote", &rec(&format!("rn{i}")), &[], Some("bench"))
            .await
            .unwrap();
        let nid = id_of(&note);
        for j in 0..3 {
            let mut tag = HashMap::new();
            tag.insert("label".to_string(), json!(format!("t{i}-{j}")));
            tag.insert("relNoteId".to_string(), json!(&nid));
            rel_client
                .create("RelTag", &tag, &[], Some("bench"))
                .await
                .unwrap();
        }
    }
    // find_many with include (resolves hasMany tags per note)
    let include_tags = vec!["tags".to_string()];
    let rel_iters = read_iters.min(200);
    let mut t = Vec::with_capacity(rel_iters);
    for _ in 0..rel_iters {
        let s = Instant::now();
        rel_client
            .find_many("RelNote", &[], &[], Some(20), Some(0), &include_tags)
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "data layer: find_many + include (20 notes × 3 tags)".to_string(),
        stats(t),
    ));

    // ----- Eventual mode: reads stay hot through writes -----
    // SAFETY: bench is single-threaded at this point; no concurrent env readers.
    unsafe { std::env::set_var("ATOMO_CACHE_MODE", "eventual") };
    let ev_atomo = atomo::Atomo::builder()
        .schema_content(
            "export interface Note { id: string; title: string; }\n\
             export const schema = { models: { Note: { tableName: 'bench_ev_notes' } } };\n\
             export default schema;",
        )
        .database_url(&db)
        .enable_migrations(true)
        .build()
        .await
        .expect("eventual atomo build");
    unsafe { std::env::remove_var("ATOMO_CACHE_MODE") };
    let ev_client = ev_atomo.client();
    let _ = sqlx::query("TRUNCATE bench_ev_notes")
        .execute(ev_atomo.db_pool())
        .await;
    for i in 0..50 {
        ev_client
            .create("Note", &rec(&format!("ev{i}")), &[], Some("bench"))
            .await
            .unwrap();
    }
    ev_client
        .find_many("Note", &[], &[], Some(20), Some(0), &[])
        .await
        .unwrap();
    // Interleaved writes + reads: cache stays hot (writes don't invalidate in eventual mode)
    let mut t = Vec::with_capacity(iters);
    for i in 0..iters {
        ev_client
            .create("Note", &rec(&format!("evw{i}")), &[], Some("bench"))
            .await
            .unwrap();
        let s = Instant::now();
        ev_client
            .find_many("Note", &[], &[], Some(20), Some(0), &[])
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "data layer: find_many eventual (hot through writes)".to_string(),
        stats(t),
    ));

    // ----- Job lease engine -----
    let pool = sqlx::PgPool::connect(&db).await.unwrap();
    let (tx, _rx) = tokio::sync::broadcast::channel(1024);
    let jobs = Arc::new(JobStore::new(pool.clone(), tx));
    jobs.init().await.unwrap();
    let queue = format!("bench-{}", uuid::Uuid::new_v4());

    let enqueue_all = |n: usize| {
        let jobs = jobs.clone();
        let q = queue.clone();
        async move {
            for _ in 0..n {
                jobs.enqueue(&q, "k", json!({}), None, 5, 0, None)
                    .await
                    .unwrap();
            }
        }
    };

    // Single-worker lease throughput (cap 50 per call until drained).
    enqueue_all(iters).await;
    let mut leased = 0usize;
    let s = Instant::now();
    loop {
        let batch = jobs
            .lease(std::slice::from_ref(&queue), 50, 60)
            .await
            .unwrap();
        if batch.is_empty() {
            break;
        }
        leased += batch.len();
    }
    let single = s.elapsed();
    rows.push((
        format!("job lease: 1 worker (drained {leased})"),
        Stats {
            n: leased,
            mean_us: single.as_micros() as f64 / leased.max(1) as f64,
            p50_us: 0,
            p95_us: 0,
            p99_us: 0,
            ops_per_s: leased as f64 / single.as_secs_f64(),
        },
    ));

    // Concurrent lease (8 workers, SKIP LOCKED — shows no lock contention).
    enqueue_all(iters).await;
    let workers = 8;
    let s = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..workers {
        let jobs = jobs.clone();
        let q = queue.clone();
        handles.push(tokio::spawn(async move {
            let mut got = 0usize;
            loop {
                let b = jobs.lease(std::slice::from_ref(&q), 50, 60).await.unwrap();
                if b.is_empty() {
                    break;
                }
                got += b.len();
            }
            got
        }));
    }
    let mut total = 0usize;
    for h in handles {
        total += h.await.unwrap();
    }
    let conc = s.elapsed();
    rows.push((
        format!("job lease: {workers} workers (drained {total})"),
        Stats {
            n: total,
            mean_us: conc.as_micros() as f64 / total.max(1) as f64,
            p50_us: 0,
            p95_us: 0,
            p99_us: 0,
            ops_per_s: total as f64 / conc.as_secs_f64(),
        },
    ));

    // ----- Plugin hook tax: JS (Javy/QuickJS) via the real hook path -----
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hook");
    let mut mgr = WasmPluginManager::new(&fixtures).unwrap();
    let load_s = Instant::now();
    let loaded = mgr.discover_and_load().await.unwrap_or_default();
    let js_load = load_s.elapsed();
    if loaded.iter().any(|p| p == "js-hook") {
        let runner = WasmHookRunner::new(Arc::new(Mutex::new(mgr)));
        for _ in 0..50 {
            let _ = runner.run_before("before_create", &make_ctx()).await;
        }
        let mut t = Vec::with_capacity(iters);
        for _ in 0..iters {
            let ctx = make_ctx();
            let s = Instant::now();
            runner.run_before("before_create", &ctx).await.unwrap();
            t.push(s.elapsed());
        }
        rows.push((
            format!(
                "plugin hook tax: JS/Javy before_create (load {:.1}ms)",
                js_load.as_secs_f64() * 1000.0
            ),
            stats(t),
        ));
    } else {
        eprintln!("(skipped JS hook bench — js-hook fixture not loaded)");
    }

    // ----- Report -----
    println!("\n## Atomo engine benchmarks (in-process)\n");
    println!("iterations: {iters} · engine-level latencies (exclude HTTP/network/GraphQL)\n");
    println!("| Benchmark | n | mean µs | p50 µs | p95 µs | p99 µs | ops/sec |");
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
    println!();
    let _ = sqlx::query("TRUNCATE bench_notes, bench_rel_notes, bench_rel_tags, bench_ev_notes")
        .execute(atomo.db_pool())
        .await;
}
