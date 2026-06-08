//! GraphQL `enqueueJob` mutation (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test jobs_graphql -- --ignored
//! Verifies the mutation requires auth, enqueues onto the durable job queue, and stamps the
//! request's tenant.

use async_graphql::Request;
use atomo::graphql::{TenantCtx, UserIdCtx};
use atomo_server::jobs::JobStore;
use sqlx::Row;

const SCHEMA_TS: &str = r#"
export interface Note { id: string; title: string; }
export const schema = { models: { Note: { tableName: 'notes' } } };
export default schema;
"#;

#[tokio::test]
#[ignore]
async fn graphql_enqueue_job_requires_auth_and_stamps_tenant() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let atomo = atomo::Atomo::builder()
        .schema_content(SCHEMA_TS)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    // Ensure the `jobs` table exists (the schema's JobStore shares this pool).
    let (tx, _rx) = tokio::sync::broadcast::channel(8);
    JobStore::new(atomo.db_pool().clone(), tx)
        .init()
        .await
        .unwrap();
    let gql = atomo_server::handlers::build_extended_schema(&atomo);

    let queue = format!("media-{}", uuid::Uuid::new_v4());
    let mutation =
        format!(r#"mutation {{ enqueueJob(queue: "{queue}", kind: "video.generate") }}"#);

    // Unauthenticated → error, no job created.
    let resp = gql.execute(Request::new(mutation.clone())).await;
    assert!(!resp.errors.is_empty(), "must require auth");
    assert!(resp.errors[0].message.contains("authentication required"));

    // Authenticated → returns a job id.
    let resp = gql
        .execute(Request::new(mutation.clone()).data(UserIdCtx("u1".to_string())))
        .await;
    assert!(resp.errors.is_empty(), "errors: {:?}", resp.errors);
    let data = serde_json::to_value(&resp.data).unwrap();
    let id = data["enqueueJob"].as_str().unwrap().to_string();

    // The job exists, queued, with the right queue/kind and no tenant.
    let row = sqlx::query("SELECT status, queue, kind, tenant_id FROM jobs WHERE id = $1")
        .bind(&id)
        .fetch_one(atomo.db_pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("status"), "queued");
    assert_eq!(row.get::<String, _>("queue"), queue);
    assert_eq!(row.get::<String, _>("kind"), "video.generate");
    assert!(row.get::<Option<String>, _>("tenant_id").is_none());

    // With a tenant in scope, the job is stamped with it.
    let resp = gql
        .execute(
            Request::new(mutation)
                .data(UserIdCtx("u1".to_string()))
                .data(TenantCtx("tenant-x".to_string())),
        )
        .await;
    let id2 = serde_json::to_value(&resp.data).unwrap()["enqueueJob"]
        .as_str()
        .unwrap()
        .to_string();
    let tenant: Option<String> = sqlx::query_scalar("SELECT tenant_id FROM jobs WHERE id = $1")
        .bind(&id2)
        .fetch_one(atomo.db_pool())
        .await
        .unwrap();
    assert_eq!(tenant.as_deref(), Some("tenant-x"));
}
