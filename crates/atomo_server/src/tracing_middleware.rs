use axum::{extract::Request, middleware::Next, response::Response};
use tracing::Instrument;

pub async fn request_tracing(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_owned();

    let span = tracing::info_span!("request", %method, %path, %request_id);

    next.run(req).instrument(span).await
}
