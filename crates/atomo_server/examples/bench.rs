//! Atomo engine micro-benchmarks. Run (release, against a Postgres):
//!   DATABASE_URL=postgres:///atomo_bench cargo run --release -p atomo_server --example bench
//! Optional: BENCH_ITERS=5000 (default 2000).
//!
//! **Honest scope:** these are **engine-level, in-process** latencies (data layer, job lease
//! engine) — they isolate component cost and deliberately exclude HTTP framing,
//! the network, and GraphQL resolution. They answer "what does each core operation cost," not
//! "requests/sec through the full stack." See `docs/guide/advanced/benchmarks.md` for method +
//! recorded results on a stated machine.

use atomo::query::{WhereClause, WhereOperator};
use atomo_server::jobs::JobStore;
use atomo_server::rate_limit::RateLimiter;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

    // ===== CRM schema benchmarks =====
    // Uses the real CRM schema shape (6 models, relations, selects, validations)
    // with bench_crm_ prefixed tables to avoid collision.
    let crm_schema = r#"
import { model, text, number, email, select, datetime, relation, url } from '@atomo-cc/schema'

export const Company = model('bench_crm_companies', {
  fields: {
    id: text().id(),
    name: text().required().min(2).max(120),
    website: url().optional(),
    domain: text().optional(),
    industry: text().optional(),
    leadCount: number().default(0),
    openDealValue: number().default(0),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
})

export const Contact = model('bench_crm_contacts', {
  fields: {
    id: text().id(),
    firstName: text().required().min(1).max(80),
    lastName: text().required().min(1).max(80),
    email: email().required(),
    phone: text().optional(),
    companyId: relation('bench_crm_companies').optional(),
    lastActivityAt: datetime().optional(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
})

export const Lead = model('bench_crm_leads', {
  fields: {
    id: text().id(),
    email: email().required(),
    source: select(['website', 'referral', 'event', 'import', 'outbound']).default('website'),
    status: select(['new', 'qualified', 'disqualified', 'converted']).default('new'),
    score: number().default(0),
    companyId: relation('bench_crm_companies').optional(),
    contactId: relation('bench_crm_contacts').optional(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
})

export const Deal = model('bench_crm_deals', {
  fields: {
    id: text().id(),
    title: text().required().min(2).max(160),
    value: number().required().min(0),
    stage: select(['prospecting', 'proposal', 'negotiation', 'won', 'lost']).default('prospecting'),
    companyId: relation('bench_crm_companies').required(),
    contactId: relation('bench_crm_contacts').optional(),
    closedAt: datetime().optional(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
})

export const Activity = model('bench_crm_activities', {
  fields: {
    id: text().id(),
    type: select(['call', 'email', 'meeting', 'note']).required(),
    note: text().optional(),
    contactId: relation('bench_crm_contacts').required(),
    dealId: relation('bench_crm_deals').optional(),
    occurredAt: datetime().defaultNow(),
    createdAt: datetime().defaultNow(),
  },
})
"#;

    let crm = atomo::Atomo::builder()
        .schema_content(crm_schema)
        .database_url(&db)
        .enable_migrations(true)
        .build()
        .await
        .expect("crm atomo build");
    let cc = crm.client();
    let _ = sqlx::query(
        "TRUNCATE bench_crm_activities, bench_crm_deals, bench_crm_leads, \
         bench_crm_contacts, bench_crm_companies CASCADE",
    )
    .execute(crm.db_pool())
    .await;

    // Seed: 20 companies for relation benchmarks
    let mut company_ids = Vec::new();
    for i in 0..20 {
        let mut d = HashMap::new();
        d.insert("name".into(), json!(format!("Bench Corp {i}")));
        d.insert("website".into(), json!(format!("https://bench{i}.com")));
        d.insert("domain".into(), json!(format!("bench{i}.com")));
        d.insert("industry".into(), json!("SaaS"));
        d.insert("leadCount".into(), json!(i));
        d.insert("openDealValue".into(), json!(i * 10000));
        let r = cc.create("Company", &d, &[], Some("bench")).await.unwrap();
        company_ids.push(id_of(&r));
    }

    // Seed: 100 contacts spread across companies
    let mut contact_ids = Vec::new();
    for i in 0..100 {
        let mut d = HashMap::new();
        d.insert("firstName".into(), json!(format!("First{i}")));
        d.insert("lastName".into(), json!(format!("Last{i}")));
        d.insert("email".into(), json!(format!("contact{i}@bench.test")));
        d.insert("companyId".into(), json!(&company_ids[i % 20]));
        let r = cc.create("Contact", &d, &[], Some("bench")).await.unwrap();
        contact_ids.push(id_of(&r));
    }

    // CRM: create Company (many fields)
    let crm_iters = iters.min(500);
    let mut t = Vec::with_capacity(crm_iters);
    for i in 0..crm_iters {
        let mut d = HashMap::new();
        d.insert("name".into(), json!(format!("CrmBench Co {i}")));
        d.insert("website".into(), json!(format!("https://crmbench{i}.io")));
        d.insert("domain".into(), json!(format!("crmbench{i}.io")));
        d.insert("industry".into(), json!("Technology"));
        d.insert("leadCount".into(), json!(0));
        d.insert("openDealValue".into(), json!(0));
        let s = Instant::now();
        cc.create("Company", &d, &[], Some("bench")).await.unwrap();
        t.push(s.elapsed());
    }
    rows.push(("crm: create Company (7 fields)".into(), stats(t)));

    // CRM: create Contact with relation FK
    let mut t = Vec::with_capacity(crm_iters);
    for i in 0..crm_iters {
        let mut d = HashMap::new();
        d.insert("firstName".into(), json!(format!("CF{i}")));
        d.insert("lastName".into(), json!(format!("CL{i}")));
        d.insert("email".into(), json!(format!("crm-c{i}@bench.test")));
        d.insert("phone".into(), json!("+1-555-0000"));
        d.insert("companyId".into(), json!(&company_ids[i % 20]));
        let s = Instant::now();
        cc.create("Contact", &d, &[], Some("bench")).await.unwrap();
        t.push(s.elapsed());
    }
    rows.push(("crm: create Contact (6 fields + FK)".into(), stats(t)));

    // CRM: create_many Lead batch
    let lead_batch = 50usize;
    let lead_batches = (crm_iters / lead_batch).max(1);
    let mut t = Vec::with_capacity(lead_batches);
    let sources = ["website", "referral", "event", "import", "outbound"];
    let statuses = ["new", "qualified", "disqualified", "converted"];
    for b in 0..lead_batches {
        let batch: Vec<_> = (0..lead_batch)
            .map(|i| {
                let mut d = HashMap::new();
                d.insert("email".into(), json!(format!("lead-{b}-{i}@bench.test")));
                d.insert("source".into(), json!(sources[i % 5]));
                d.insert("status".into(), json!(statuses[i % 4]));
                d.insert("score".into(), json!((i * 7) % 100));
                d.insert("companyId".into(), json!(&company_ids[i % 20]));
                d.insert("contactId".into(), json!(&contact_ids[i % 100]));
                d
            })
            .collect();
        let s = Instant::now();
        cc.create_many("Lead", &batch, Some("bench")).await.unwrap();
        t.push(s.elapsed() / lead_batch as u32);
    }
    rows.push((
        format!("crm: create_many Lead (per row, batch={lead_batch})"),
        stats(t),
    ));

    // CRM: find_many Lead filtered by status (hot)
    let status_filter = vec![WhereClause {
        field: "status".to_string(),
        operator: WhereOperator::Equals,
        value: json!("qualified"),
    }];
    cc.find_many("Lead", &status_filter, &[], Some(20), Some(0), &[])
        .await
        .unwrap();
    let mut t = Vec::with_capacity(iters);
    for _ in 0..iters {
        let s = Instant::now();
        cc.find_many("Lead", &status_filter, &[], Some(20), Some(0), &[])
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "crm: find_many Lead WHERE status='qualified' hot".into(),
        stats(t),
    ));

    // CRM: find_many Contact by companyId (hot)
    let company_filter = vec![WhereClause {
        field: "companyId".to_string(),
        operator: WhereOperator::Equals,
        value: json!(&company_ids[0]),
    }];
    cc.find_many("Contact", &company_filter, &[], Some(20), Some(0), &[])
        .await
        .unwrap();
    let mut t = Vec::with_capacity(iters);
    for _ in 0..iters {
        let s = Instant::now();
        cc.find_many("Contact", &company_filter, &[], Some(20), Some(0), &[])
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "crm: find_many Contact WHERE companyId=X hot".into(),
        stats(t),
    ));

    // CRM: find_unique Contact by id (hot)
    let contact_by_id = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: json!(&contact_ids[0]),
    }];
    cc.find_unique("Contact", &contact_by_id, &[])
        .await
        .unwrap();
    let mut t = Vec::with_capacity(iters);
    for _ in 0..iters {
        let s = Instant::now();
        cc.find_unique("Contact", &contact_by_id, &[]).await.unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "crm: find_unique Contact by id hot".into(),
        stats(t),
    ));

    // CRM: mixed workload (create Deal + read Leads — simulates real app)
    let mix_iters = crm_iters.min(200);
    let mut t = Vec::with_capacity(mix_iters);
    for i in 0..mix_iters {
        let mut d = HashMap::new();
        d.insert("title".into(), json!(format!("Deal {i}")));
        d.insert("value".into(), json!(50000 + i * 1000));
        d.insert("stage".into(), json!("prospecting"));
        d.insert("companyId".into(), json!(&company_ids[i % 20]));
        d.insert("contactId".into(), json!(&contact_ids[i % 100]));
        let s = Instant::now();
        cc.create("Deal", &d, &[], Some("bench")).await.unwrap();
        cc.find_many("Lead", &status_filter, &[], Some(20), Some(0), &[])
            .await
            .unwrap();
        t.push(s.elapsed());
    }
    rows.push((
        "crm: mixed (create Deal + find_many Lead)".into(),
        stats(t),
    ));

    // ----- GraphQL full-stack round-trip -----
    // Measures the actual cost consumers pay: GraphQL resolution + casing conversion +
    // validation + engine. Uses schema.execute() in-process (no HTTP/network).
    let gql_schema_content =
        "export interface GqlNote { id: string; title: string; }\n\
         export const schema = { models: { GqlNote: { tableName: 'bench_gql_notes' } } };\n\
         export default schema;";
    let gql_atomo = atomo::Atomo::builder()
        .schema_content(gql_schema_content)
        .database_url(&db)
        .enable_migrations(true)
        .build()
        .await
        .expect("gql atomo build");
    let gql_client = Arc::new(gql_atomo.client().clone());
    let gql_pool = gql_atomo.db_pool().clone();
    let gql = atomo::graphql::build_schema(gql_client.clone(), gql_atomo.schema(), gql_pool.clone());
    let _ = sqlx::query("TRUNCATE bench_gql_notes")
        .execute(gql_atomo.db_pool())
        .await;

    // Warm: seed some records
    for i in 0..50 {
        let q = format!(
            r#"mutation {{ create(model: "GqlNote", data: {{ title: "warm{i}" }}) }}"#
        );
        gql.execute(async_graphql::Request::new(&q)).await;
    }

    // GraphQL create mutation
    let gql_iters = iters.min(500);
    let mut t = Vec::with_capacity(gql_iters);
    for i in 0..gql_iters {
        let q = format!(
            r#"mutation {{ create(model: "GqlNote", data: {{ title: "gql{i}" }}) }}"#
        );
        let req = async_graphql::Request::new(&q);
        let s = Instant::now();
        gql.execute(req).await;
        t.push(s.elapsed());
    }
    rows.push(("graphql: create mutation (full stack)".into(), stats(t)));

    // GraphQL records query (hot)
    gql.execute(async_graphql::Request::new(
        r#"{ records(model: "GqlNote", limit: 20) }"#,
    ))
    .await;
    let mut t = Vec::with_capacity(iters);
    for _ in 0..iters {
        let req = async_graphql::Request::new(
            r#"{ records(model: "GqlNote", limit: 20) }"#,
        );
        let s = Instant::now();
        gql.execute(req).await;
        t.push(s.elapsed());
    }
    rows.push((
        "graphql: records query hot (limit 20, incl. camelCase)".into(),
        stats(t),
    ));

    // GraphQL record by id (hot)
    let gql_seed = gql
        .execute(async_graphql::Request::new(
            r#"mutation { create(model: "GqlNote", data: { title: "gql-unique" }) }"#,
        ))
        .await;
    let gql_seed_id = gql_seed
        .data
        .into_json()
        .ok()
        .and_then(|v| v.get("create").and_then(|c| c.get("id")).and_then(|id| id.as_str().map(String::from)))
        .unwrap_or_default();
    if !gql_seed_id.is_empty() {
        let q = format!(r#"{{ record(model: "GqlNote", id: "{gql_seed_id}") }}"#);
        gql.execute(async_graphql::Request::new(&q)).await;
        let mut t = Vec::with_capacity(iters);
        for _ in 0..iters {
            let req = async_graphql::Request::new(&q);
            let s = Instant::now();
            gql.execute(req).await;
            t.push(s.elapsed());
        }
        rows.push(("graphql: record by id hot".into(), stats(t)));
    }

    // ----- updateMany: N sequential vs 1 bulk call -----
    // Create N records, then compare N individual update mutations vs one updateMany.
    let bulk_n = 50usize;
    let mut seed_ids = Vec::with_capacity(bulk_n);
    for i in 0..bulk_n {
        let q = format!(
            r#"mutation {{ create(model: "GqlNote", data: {{ title: "bulk{i}" }}) }}"#
        );
        let resp = gql.execute(async_graphql::Request::new(&q)).await;
        if let Ok(v) = resp.data.into_json() {
            if let Some(id) = v.get("create").and_then(|c| c.get("id")).and_then(|id| id.as_str()) {
                seed_ids.push(id.to_string());
            }
        }
    }

    // N sequential updates
    let rounds = 5;
    let mut t = Vec::with_capacity(rounds);
    for r in 0..rounds {
        let s = Instant::now();
        for (i, id) in seed_ids.iter().enumerate() {
            let q = format!(
                r#"mutation {{ update(model: "GqlNote", id: "{id}", data: {{ title: "seq-{r}-{i}" }}) }}"#
            );
            gql.execute(async_graphql::Request::new(&q)).await;
        }
        t.push(s.elapsed() / bulk_n as u32);
    }
    rows.push((
        format!("graphql: update ×{bulk_n} sequential (per row)"),
        stats(t),
    ));

    // 1 updateMany call
    let mut t = Vec::with_capacity(rounds);
    for r in 0..rounds {
        let entries: Vec<String> = seed_ids
            .iter()
            .enumerate()
            .map(|(i, id)| format!(r#"{{ id: "{id}", data: {{ title: "bulk-{r}-{i}" }} }}"#))
            .collect();
        let q = format!(
            r#"mutation {{ updateMany(model: "GqlNote", updates: [{}]) }}"#,
            entries.join(", ")
        );
        let s = Instant::now();
        gql.execute(async_graphql::Request::new(&q)).await;
        t.push(s.elapsed() / bulk_n as u32);
    }
    rows.push((
        format!("graphql: updateMany ×{bulk_n} bulk (per row)"),
        stats(t),
    ));

    // ----- Rate limiter throughput -----
    let rl = RateLimiter::new(1_000_000, 60);
    let rl_iters = iters.max(5000);
    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    // Single-threaded throughput
    let mut t = Vec::with_capacity(rl_iters);
    for _ in 0..rl_iters {
        let s = Instant::now();
        let _ = rl.check(ip).await;
        t.push(s.elapsed());
    }
    rows.push((
        "rate limiter: check (single IP, serial)".into(),
        stats(t),
    ));

    // Concurrent throughput: 8 tasks, unique IPs
    let conc_workers = 8u8;
    let per_worker = (rl_iters / conc_workers as usize).max(100);
    let rl2 = RateLimiter::new(1_000_000, 60);
    let s = Instant::now();
    let mut handles = Vec::new();
    for w in 0..conc_workers {
        let rl = rl2.clone();
        handles.push(tokio::spawn(async move {
            let ip = IpAddr::V4(Ipv4Addr::new(10, 0, w, 1));
            for _ in 0..per_worker {
                let _ = rl.check(ip).await;
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    let total_checks = per_worker * conc_workers as usize;
    let conc_elapsed = s.elapsed();
    rows.push((
        format!("rate limiter: check ({conc_workers} concurrent IPs, {total_checks} total)"),
        Stats {
            n: total_checks,
            mean_us: conc_elapsed.as_micros() as f64 / total_checks as f64,
            p50_us: 0,
            p95_us: 0,
            p99_us: 0,
            ops_per_s: total_checks as f64 / conc_elapsed.as_secs_f64(),
        },
    ));

    // ----- Report -----
    println!("\n## Atomo engine benchmarks (in-process)\n");
    println!("iterations: {iters} · engine-level + GraphQL-level latencies (exclude HTTP/network)\n");
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
    let _ = sqlx::query("TRUNCATE bench_notes, bench_rel_notes, bench_rel_tags, bench_ev_notes, bench_gql_notes")
        .execute(atomo.db_pool())
        .await;
    let _ = sqlx::query(
        "TRUNCATE bench_crm_activities, bench_crm_deals, bench_crm_leads, \
         bench_crm_contacts, bench_crm_companies CASCADE",
    )
    .execute(crm.db_pool())
    .await;
}
