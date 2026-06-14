//! Narrow service-authenticated command surface for public Demo runs.

use crate::jobs::JobStore;
use axum::{
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

fn signature_hex(secret: &str, timestamp: &str, method: &str, path: &str, body: &str) -> String {
    let mut key = secret.as_bytes().to_vec();
    if key.len() > 64 {
        key = Sha256::digest(&key).to_vec();
    }
    key.resize(64, 0);
    let outer: Vec<u8> = key.iter().map(|byte| byte ^ 0x5c).collect();
    let inner: Vec<u8> = key.iter().map(|byte| byte ^ 0x36).collect();
    let message = format!("{timestamp}\n{method}\n{path}\n{body}");
    let inner_hash = Sha256::new()
        .chain_update(inner)
        .chain_update(message.as_bytes())
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

fn authorized(headers: &HeaderMap, method: &str, path: &str, body: &str) -> bool {
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

fn configured_limit(name: &str, fallback: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn queue_has_capacity(queued: i64, leased: i64, max_queued: i64, max_concurrent: i64) -> bool {
    queued < max_queued && leased < max_concurrent
}

async fn create_run(
    State(state): State<PublicDemoState>,
    headers: HeaderMap,
    Json(request): Json<CreatePublicDemoRun>,
) -> Response {
    let body = serde_json::to_string(&request).unwrap_or_default();
    if !authorized(&headers, "POST", "/internal/public-demo/runs", &body) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !preset_allowed(&request.preset_id)
        || request.listing_slug.is_empty()
        || request.publication_version < 1
        || request.source_asset_token.is_empty()
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

    let token = format!(
        "run_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let token_hash = sha256_hex(&token);
    let payload = json!({
        "publicListingId": listing_id,
        "publicationVersion": request.publication_version,
        "presetId": request.preset_id,
        "sourceAssetToken": request.source_asset_token,
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
          anonymous_session_hash, ip_hash, status, source_delete_after, result_delete_after)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'queued', NOW() + INTERVAL '1 hour',
                 NOW() + INTERVAL '24 hours')",
    )
    .bind(record_id)
    .bind(&token_hash)
    .bind(job_id)
    .bind(listing_id)
    .bind(request.publication_version as f64)
    .bind(request.anonymous_session_hash)
    .bind(request.ip_hash)
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
    if !authorized(&headers, "GET", &path, "") || token_hash.len() != 64 {
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
    Json(json!({
        "status": status,
        "stage": stage,
        "resultUrl": result_url,
        "errorCode": error_code,
    }))
    .into_response()
}

pub fn public_demo_router(jobs: Arc<JobStore>, pool: PgPool) -> Router {
    Router::new()
        .route("/internal/public-demo/runs", post(create_run))
        .route(
            "/internal/public-demo/runs/by-token-hash/{token_hash}",
            get(run_status),
        )
        .with_state(PublicDemoState { jobs, pool })
}

#[cfg(test)]
mod tests {
    use super::{
        constant_time_eq, preset_allowed, queue_has_capacity, sha256_hex, signature_hex,
    };

    #[test]
    fn token_hash_is_stable_and_not_plaintext() {
        let hash = sha256_hex("run_secret");
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, "run_secret");
    }

    #[test]
    fn signature_is_bound_to_path_and_body() {
        let a = signature_hex("a sufficiently long secret value", "1", "POST", "/a", "{}");
        let b = signature_hex("a sufficiently long secret value", "1", "POST", "/b", "{}");
        let c = signature_hex("a sufficiently long secret value", "1", "POST", "/a", "{\"x\":1}");
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
}
