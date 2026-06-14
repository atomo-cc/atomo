//! Narrow service-authenticated command surface for public Demo runs.

use crate::{jobs::JobStore, media::MediaState};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{sync::Arc, time::SystemTime};

#[derive(Clone)]
struct PublicDemoState {
    jobs: Arc<JobStore>,
    pool: PgPool,
    media: Arc<MediaState>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePublicDemoRun {
    listing_slug: String,
    publication_version: i32,
    preset_id: String,
    source_asset_token: String,
    anonymous_session_hash: String,
    ip_hash: String,
}

fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn signature_hex(secret: &str, timestamp: &str, method: &str, path: &str, body: &[u8]) -> String {
    let mut key = secret.as_bytes().to_vec();
    if key.len() > 64 {
        key = Sha256::digest(&key).to_vec();
    }
    key.resize(64, 0);
    let outer: Vec<u8> = key.iter().map(|byte| byte ^ 0x5c).collect();
    let inner: Vec<u8> = key.iter().map(|byte| byte ^ 0x36).collect();
    let inner_hash = Sha256::new()
        .chain_update(inner)
        .chain_update(format!("{timestamp}\n{method}\n{path}\n").as_bytes())
        .chain_update(body)
        .finalize();
    Sha256::new()
        .chain_update(outer)
        .chain_update(inner_hash)
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

fn authorized(headers: &HeaderMap, method: &str, path: &str, body: &[u8]) -> bool {
    let Ok(secret) = std::env::var("ATOMO_PUBLIC_DEMO_HMAC_SECRET") else {
        return false;
    };
    if secret.len() < 32 {
        return false;
    }
    let Some(timestamp) = headers
        .get("x-aicreatory-timestamp")
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some(presented) = headers
        .get("x-aicreatory-signature")
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Ok(unix) = timestamp.parse::<u64>() else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    if now.abs_diff(unix) > 300 {
        return false;
    }
    constant_time_eq(
        &signature_hex(&secret, timestamp, method, path, body),
        &presented.to_ascii_lowercase(),
    )
}

fn preset_allowed(preset: &str) -> bool {
    std::env::var("ATOMO_PUBLIC_DEMO_PRESETS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .any(|value| !value.is_empty() && value == preset)
}

fn valid_opaque_token(value: &str) -> bool {
    (32..=256).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn configured_limit(name: &str, fallback: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn configured_budget(name: &str) -> Option<f64> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn queue_has_capacity(queued: i64, leased: i64, max_queued: i64, max_concurrent: i64) -> bool {
    queued < max_queued && leased < max_concurrent
}

fn budget_has_capacity(spent: f64, estimated_cost: f64, limit: f64) -> bool {
    spent.is_finite()
        && estimated_cost.is_finite()
        && limit.is_finite()
        && spent >= 0.0
        && estimated_cost > 0.0
        && spent + estimated_cost <= limit
}

async fn create_run(
    State(state): State<PublicDemoState>,
    headers: HeaderMap,
    Json(request): Json<CreatePublicDemoRun>,
) -> Response {
    let body = serde_json::to_string(&request).unwrap_or_default();
    if !authorized(
        &headers,
        "POST",
        "/internal/public-demo/runs",
        body.as_bytes(),
    ) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !preset_allowed(&request.preset_id)
        || request.listing_slug.is_empty()
        || request.publication_version < 1
        || !valid_opaque_token(&request.source_asset_token)
        || request.anonymous_session_hash.len() != 64
        || request.ip_hash.len() != 64
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Some(listing) = sqlx::query(
        "SELECT id, publication_version FROM public_listings
         WHERE slug = $1 AND status = 'published' AND quality_status = 'approved'
           AND demo_status = 'anonymous' AND publication_version = $2
           AND deleted_at IS NULL
         LIMIT 1",
    )
    .bind(&request.listing_slug)
    .bind(request.publication_version as f64)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let listing_id: String = listing.get("id");
    let queue = std::env::var("ATOMO_PUBLIC_DEMO_QUEUE").unwrap_or_default();
    let kind = std::env::var("ATOMO_PUBLIC_DEMO_KIND").unwrap_or_default();
    if queue.is_empty() || kind.is_empty() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let duplicate = sqlx::query(
        "SELECT 1 FROM public_demo_runs r
         JOIN jobs j ON j.id = r.internal_job_id
         WHERE (r.anonymous_session_hash = $1 OR r.ip_hash = $2)
           AND r.created_at > NOW() - INTERVAL '24 hours'
           AND r.deleted_at IS NULL
           AND j.status IN ('queued', 'leased', 'succeeded')
         LIMIT 1",
    )
    .bind(&request.anonymous_session_hash)
    .bind(&request.ip_hash)
    .fetch_optional(&state.pool)
    .await;
    match duplicate {
        Ok(Some(_)) => return StatusCode::TOO_MANY_REQUESTS.into_response(),
        Ok(None) => {}
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let queue_counts = sqlx::query(
        "SELECT
           COUNT(*) FILTER (WHERE status = 'queued') AS queued,
           COUNT(*) FILTER (WHERE status = 'leased') AS leased
         FROM jobs WHERE queue = $1",
    )
    .bind(&queue)
    .fetch_one(&state.pool)
    .await;
    let Ok(queue_counts) = queue_counts else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if !queue_has_capacity(
        queue_counts.get("queued"),
        queue_counts.get("leased"),
        configured_limit("ATOMO_PUBLIC_DEMO_MAX_QUEUED", 20),
        configured_limit("ATOMO_PUBLIC_DEMO_MAX_CONCURRENT", 5),
    ) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let Some(estimated_cost) = configured_budget("ATOMO_PUBLIC_DEMO_ESTIMATED_RUN_COST") else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(hourly_budget) = configured_budget("ATOMO_PUBLIC_DEMO_HOURLY_BUDGET") else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(daily_budget) = configured_budget("ATOMO_PUBLIC_DEMO_DAILY_BUDGET") else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(app_daily_budget) = configured_budget("ATOMO_PUBLIC_DEMO_APP_DAILY_BUDGET") else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let spend = sqlx::query(
        "SELECT
           COALESCE(SUM(COALESCE(actual_cost, estimated_cost)) FILTER (
             WHERE created_at > NOW() - INTERVAL '1 hour'
           ), 0) AS hourly,
           COALESCE(SUM(COALESCE(actual_cost, estimated_cost)) FILTER (
             WHERE created_at > NOW() - INTERVAL '24 hours'
           ), 0) AS daily,
           COALESCE(SUM(COALESCE(actual_cost, estimated_cost)) FILTER (
             WHERE created_at > NOW() - INTERVAL '24 hours' AND public_listing_id = $1
           ), 0) AS app_daily,
           COUNT(*) FILTER (
             WHERE created_at > NOW() - INTERVAL '1 hour' AND actual_cost > $2
           ) AS over_hard_limit
         FROM public_demo_runs WHERE deleted_at IS NULL",
    )
    .bind(&listing_id)
    .bind(configured_budget("ATOMO_PUBLIC_DEMO_HARD_RUN_COST").unwrap_or(0.08))
    .fetch_one(&state.pool)
    .await;
    let Ok(spend) = spend else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if spend.get::<i64, _>("over_hard_limit") > 0
        || !budget_has_capacity(spend.get("hourly"), estimated_cost, hourly_budget)
        || !budget_has_capacity(spend.get("daily"), estimated_cost, daily_budget)
        || !budget_has_capacity(spend.get("app_daily"), estimated_cost, app_daily_budget)
    {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    let token = format!(
        "run_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let token_hash = sha256_hex(&token);
    let source_token_hash = sha256_hex(&request.source_asset_token);
    let source_asset = sqlx::query(
        "UPDATE public_demo_uploads SET consumed_at = NOW(), updated_at = NOW()
         WHERE token_hash = $1 AND consumed_at IS NULL AND expires_at > NOW()
           AND deleted_at IS NULL
         RETURNING asset_id",
    )
    .bind(source_token_hash)
    .fetch_optional(&state.pool)
    .await;
    let Ok(Some(source_asset)) = source_asset else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let source_asset_id: String = source_asset.get("asset_id");
    let payload = json!({
        "publicListingId": listing_id,
        "publicationVersion": request.publication_version,
        "presetId": request.preset_id,
        "sourceAssetId": source_asset_id,
    });
    let job_id = match state
        .jobs
        .enqueue(&queue, &kind, payload, Some(&token_hash), 2, 0, None)
        .await
    {
        Ok(id) => id,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let record_id = uuid::Uuid::new_v4().to_string();
    let inserted = sqlx::query(
        "INSERT INTO public_demo_runs
         (id, token_hash, internal_job_id, public_listing_id, publication_version,
          anonymous_session_hash, ip_hash, status, estimated_cost, source_delete_after,
          result_delete_after)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'queued', $8, NOW() + INTERVAL '1 hour',
                 NOW() + INTERVAL '24 hours')",
    )
    .bind(record_id)
    .bind(&token_hash)
    .bind(job_id)
    .bind(listing_id)
    .bind(request.publication_version as f64)
    .bind(request.anonymous_session_hash)
    .bind(request.ip_hash)
    .bind(estimated_cost)
    .execute(&state.pool)
    .await;
    if inserted.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    (
        StatusCode::CREATED,
        Json(json!({ "token": token, "status": "queued", "stage": "uploading" })),
    )
        .into_response()
}

async fn run_status(
    State(state): State<PublicDemoState>,
    headers: HeaderMap,
    Path(token_hash): Path<String>,
) -> Response {
    let path = format!("/internal/public-demo/runs/by-token-hash/{token_hash}");
    if !authorized(&headers, "GET", &path, b"") || token_hash.len() != 64 {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(row) = sqlx::query(
        "SELECT internal_job_id, result_expires_at FROM public_demo_runs
         WHERE token_hash = $1 AND result_delete_after > NOW() AND deleted_at IS NULL LIMIT 1",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let job_id: String = row.get("internal_job_id");
    let Ok(Some(job)) = state.jobs.get(&job_id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (status, stage, result_url, error_code) = match job.status.as_str() {
        "succeeded" => (
            "completed",
            "completed",
            job.result
                .as_ref()
                .and_then(|value| value.get("resultUrl"))
                .and_then(Value::as_str),
            None,
        ),
        "dead" => ("failed", "failed", None, Some("generation_failed")),
        "leased" => ("running", "generating_scene", None, None),
        _ => ("queued", "analyzing_product", None, None),
    };
    let result_asset_id = job
        .result
        .as_ref()
        .and_then(|value| value.get("assetId"))
        .and_then(Value::as_str);
    let actual_cost = job
        .result
        .as_ref()
        .and_then(|value| value.get("actualCost"))
        .and_then(Value::as_f64);
    let _ = sqlx::query(
        "UPDATE public_demo_runs
         SET status = $2, sanitized_stage = $3, sanitized_error_code = $4,
             result_asset_id = COALESCE($5, result_asset_id),
             actual_cost = COALESCE($6, actual_cost), updated_at = NOW()
         WHERE token_hash = $1",
    )
    .bind(&token_hash)
    .bind(status)
    .bind(stage)
    .bind(error_code)
    .bind(result_asset_id)
    .bind(actual_cost)
    .execute(&state.pool)
    .await;
    Json(json!({
        "status": status,
        "stage": stage,
        "resultUrl": result_url,
        "errorCode": error_code,
    }))
    .into_response()
}

async fn upload_source(
    State(state): State<PublicDemoState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !authorized(&headers, "POST", "/internal/public-demo/uploads", &body) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if body.is_empty() || body.len() > state.media.max_size() {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let session_hash = headers
        .get("x-aicreatory-session-hash")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let ip_hash = headers
        .get("x-aicreatory-ip-hash")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if session_hash.len() != 64 || ip_hash.len() != 64 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let upload_count = sqlx::query(
        "SELECT COUNT(*) AS count FROM public_demo_uploads
         WHERE (anonymous_session_hash = $1 OR ip_hash = $2)
           AND created_at > NOW() - INTERVAL '1 hour' AND deleted_at IS NULL",
    )
    .bind(session_hash)
    .bind(ip_hash)
    .fetch_one(&state.pool)
    .await;
    match upload_count {
        Ok(row) if row.get::<i64, _>("count") >= 3 => {
            return StatusCode::TOO_MANY_REQUESTS.into_response()
        }
        Ok(_) => {}
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !matches!(content_type, "image/jpeg" | "image/png" | "image/webp")
        || !crate::media::content_type_allowed(content_type)
        || !crate::media::content_matches(content_type, &body)
    {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let filename = headers
        .get("x-aicreatory-filename")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 200)
        .unwrap_or("public-demo-upload");
    let (asset_id, _) = match state
        .media
        .store_upload(filename, content_type, &body, "system:public-demo", None)
        .await
    {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let token = format!(
        "asset_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let inserted = sqlx::query(
        "INSERT INTO public_demo_uploads
         (id, token_hash, asset_id, anonymous_session_hash, ip_hash, content_type, size, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW() + INTERVAL '1 hour')",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(sha256_hex(&token))
    .bind(asset_id)
    .bind(session_hash)
    .bind(ip_hash)
    .bind(content_type)
    .bind(body.len() as f64)
    .execute(&state.pool)
    .await;
    if inserted.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    (
        StatusCode::CREATED,
        Json(json!({ "sourceAssetToken": token, "expiresInSeconds": 3600 })),
    )
        .into_response()
}

pub fn public_demo_router(jobs: Arc<JobStore>, pool: PgPool, media: Arc<MediaState>) -> Router {
    Router::new()
        .route("/internal/public-demo/uploads", post(upload_source))
        .route("/internal/public-demo/runs", post(create_run))
        .route(
            "/internal/public-demo/runs/by-token-hash/{token_hash}",
            get(run_status),
        )
        .with_state(PublicDemoState { jobs, pool, media })
}

#[cfg(test)]
mod tests {
    use super::{
        budget_has_capacity, constant_time_eq, preset_allowed, queue_has_capacity, sha256_hex,
        signature_hex, valid_opaque_token,
    };

    #[test]
    fn token_hash_is_stable_and_not_plaintext() {
        let hash = sha256_hex("run_secret");
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, "run_secret");
    }

    #[test]
    fn signature_is_bound_to_path_and_body() {
        let a = signature_hex("a sufficiently long secret value", "1", "POST", "/a", b"{}");
        let b = signature_hex("a sufficiently long secret value", "1", "POST", "/b", b"{}");
        let c = signature_hex("a sufficiently long secret value", "1", "POST", "/a", b"{\"x\":1}");
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn signature_comparison_is_exact() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "ab"));
    }

    #[test]
    fn empty_preset_configuration_denies_everything() {
        std::env::remove_var("ATOMO_PUBLIC_DEMO_PRESETS");
        assert!(!preset_allowed("studio"));
    }

    #[test]
    fn queue_capacity_enforces_both_limits() {
        assert!(queue_has_capacity(19, 4, 20, 5));
        assert!(!queue_has_capacity(20, 0, 20, 5));
        assert!(!queue_has_capacity(0, 5, 20, 5));
    }

    #[test]
    fn budget_capacity_includes_next_run() {
        assert!(budget_has_capacity(9.90, 0.05, 10.0));
        assert!(budget_has_capacity(9.95, 0.05, 10.0));
        assert!(!budget_has_capacity(9.96, 0.05, 10.0));
        assert!(!budget_has_capacity(0.0, 0.0, 10.0));
    }

    #[test]
    fn source_asset_token_is_opaque_not_a_url() {
        assert!(valid_opaque_token(
            "asset_0123456789abcdef0123456789abcdef"
        ));
        assert!(!valid_opaque_token("https://internal.example/file"));
        assert!(!valid_opaque_token("../../private-file"));
        assert!(!valid_opaque_token("short"));
    }
}
