use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};

fn rl_app(max: u32) -> Router {
    let limiter = atomo_server::rate_limit::RateLimiter::new(max, 60);
    Router::new()
        .route("/ping", get(|| async { "pong" }))
        .route_layer(axum::middleware::from_fn_with_state(
            limiter,
            atomo_server::rate_limit::rate_limit_middleware,
        ))
}

async fn status(app: &Router, ip: &str) -> StatusCode {
    let req = Request::builder()
        .uri("/ping")
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn rate_limit_allows_under_limit() {
    let app = rl_app(3);
    for _ in 0..3 {
        assert_eq!(status(&app, "1.2.3.4").await, StatusCode::OK);
    }
}

#[tokio::test]
async fn rate_limit_blocks_over_limit() {
    let app = rl_app(2);
    assert_eq!(status(&app, "5.6.7.8").await, StatusCode::OK);
    assert_eq!(status(&app, "5.6.7.8").await, StatusCode::OK);
    assert_eq!(status(&app, "5.6.7.8").await, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn rate_limit_is_per_ip() {
    let app = rl_app(1);
    assert_eq!(status(&app, "9.9.9.9").await, StatusCode::OK);
    assert_eq!(status(&app, "9.9.9.9").await, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(status(&app, "8.8.8.8").await, StatusCode::OK);
}

#[tokio::test]
async fn cors_layer_adds_headers() {
    let app = Router::new()
        .route("/x", get(|| async { "ok" }))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any));
    let req = Request::builder().uri("/x").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "*"
    );
}
