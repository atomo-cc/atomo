//! Phase-3 transactional route, end to end over HTTP: router → JS (Javy) plugin →
//! atomic transaction. Requires Postgres via DATABASE_URL and the Javy-built fixture
//! (tests/fixtures/route-billing/billing/plugin.wasm).
//! Run: cargo test -p atomo_server --test route_transaction -- --ignored

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tokio::sync::Mutex;
use tower::ServiceExt;

async fn balance(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_as::<_, (i64,)>("SELECT balance FROM accounts WHERE user_id='u1'")
        .fetch_one(pool)
        .await
        .unwrap()
        .0
}

#[tokio::test]
#[ignore]
async fn transactional_route_debits_atomically_over_http() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS accounts")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE accounts (user_id TEXT PRIMARY KEY, balance BIGINT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO accounts (user_id, balance) VALUES ('u1', 10)")
        .execute(&pool)
        .await
        .unwrap();

    // Load the billing fixture plugin (declares POST /ext/billing/debit, auth=false).
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/route-billing");
    let mut mgr = atomo_server::wasm_plugins::WasmPluginManager::new(dir).unwrap();
    mgr.discover_and_load().await.unwrap();
    let routes = mgr.plugin_routes();
    assert!(!routes.is_empty(), "fixture must declare a route");
    let mgr = Arc::new(Mutex::new(mgr));

    let auth = atomo_server::auth::HttpAuthService::new("test-secret", pool.clone());
    let app = atomo_server::plugin_routes::plugin_routes_router(mgr, auth, pool.clone(), routes);

    // Sufficient debit → 200 "applied"; balance 10 - 4 = 6 (transaction committed).
    let req = Request::builder()
        .method("POST")
        .uri("/ext/billing/debit")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"userId":"u1","cost":4,"idempotencyKey":"k1"}"#,
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "sufficient debit should be 200"
    );
    assert_eq!(balance(&pool).await, 6, "sufficient debit should commit");

    // Insufficient debit → 402 (the handler's elseStatus); balance unchanged (rolled back).
    let req2 = Request::builder()
        .method("POST")
        .uri("/ext/billing/debit")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"userId":"u1","cost":100,"idempotencyKey":"k2"}"#,
        ))
        .unwrap();
    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(
        resp2.status(),
        StatusCode::PAYMENT_REQUIRED,
        "insufficient debit should return the handler's 402 else-response"
    );
    assert_eq!(balance(&pool).await, 6, "insufficient debit must roll back");

    sqlx::query("DROP TABLE IF EXISTS accounts")
        .execute(&pool)
        .await
        .unwrap();
}
