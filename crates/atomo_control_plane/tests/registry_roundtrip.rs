//! ProjectRegistry CRUD round-trip against a real Postgres — proves the data layer persists and
//! reloads every field (incl. the `schema_ref` JSONB enum, `aliases` array, and status/desired
//! enums) and that lookups/mutations/delete behave.
//!
//! Requires DATABASE_URL. Creates the registry tables (idempotent) and cleans its own test row.
//! Run: cargo test -p atomo_control_plane --test registry_roundtrip -- --ignored

use atomo_control_plane::registry::{
    DesiredState, Project, ProjectRegistry, ProjectStatus, SchemaRef,
};

const ID: &str = "ctlplane_roundtrip_test";

fn sample(now: chrono::DateTime<chrono::Utc>) -> Project {
    Project {
        id: ID.to_string(),
        display_name: "Round Trip".into(),
        hostname: Some("rt.example.com".into()),
        aliases: vec!["rt-alias.example.com".into()],
        database_url_ref: "/atomo/rt/DATABASE_URL".into(),
        schema_ref: SchemaRef::Git {
            repo: "git@example.com:org/schemas.git".into(),
            path: "projects/rt/schema.ts".into(),
            git_ref: "deadbeef".into(),
        },
        schema_version: None,
        upstream: None,
        env: serde_json::json!({ "FEATURE_X": "on" }),
        status: ProjectStatus::Provisioning,
        desired_state: DesiredState::Running,
        last_health: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
#[ignore]
async fn registry_crud_roundtrip() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let reg = ProjectRegistry::new(pool.clone());
    reg.init().await.unwrap();

    // Idempotent: clear any leftover from a previous run.
    sqlx::query("DELETE FROM project_events WHERE project_id = $1")
        .bind(ID)
        .execute(&pool)
        .await
        .ok();
    reg.delete(ID).await.ok();

    // Create → get: every field survives the JSONB/array/enum round-trip.
    let p = sample(chrono::Utc::now());
    reg.create(&p).await.unwrap();

    let got = reg.get(ID).await.unwrap();
    assert_eq!(got.id, ID);
    assert_eq!(got.display_name, "Round Trip");
    assert_eq!(got.aliases, vec!["rt-alias.example.com".to_string()]);
    assert_eq!(got.status, ProjectStatus::Provisioning);
    assert_eq!(got.desired_state, DesiredState::Running);
    match got.schema_ref {
        SchemaRef::Git { git_ref, path, .. } => {
            assert_eq!(git_ref, "deadbeef");
            assert_eq!(path, "projects/rt/schema.ts");
        }
        other => panic!("schema_ref did not round-trip as Git: {other:?}"),
    }
    assert_eq!(got.env["FEATURE_X"], serde_json::json!("on"));

    // list + hostname/alias resolution.
    assert!(reg.list().await.unwrap().iter().any(|x| x.id == ID));
    assert_eq!(
        reg.resolve_by_hostname("rt.example.com")
            .await
            .unwrap()
            .map(|x| x.id),
        Some(ID.to_string())
    );
    assert_eq!(
        reg.resolve_by_hostname("rt-alias.example.com")
            .await
            .unwrap()
            .map(|x| x.id),
        Some(ID.to_string()),
        "alias must resolve too"
    );
    assert!(reg
        .resolve_by_hostname("nope.example.com")
        .await
        .unwrap()
        .is_none());

    // Mutations reflect on reload.
    reg.set_upstream(ID, Some("127.0.0.1:4321")).await.unwrap();
    reg.update_status(ID, ProjectStatus::Running).await.unwrap();
    reg.set_desired_state(ID, DesiredState::Stopped)
        .await
        .unwrap();
    reg.set_schema_version(ID, "cafef00d").await.unwrap();
    reg.set_last_health(ID, serde_json::json!({ "status": "healthy" }))
        .await
        .unwrap();
    let got = reg.get(ID).await.unwrap();
    assert_eq!(got.upstream.as_deref(), Some("127.0.0.1:4321"));
    assert_eq!(got.status, ProjectStatus::Running);
    assert_eq!(got.desired_state, DesiredState::Stopped);
    assert_eq!(got.schema_version.as_deref(), Some("cafef00d"));
    assert_eq!(
        got.last_health.unwrap()["status"],
        serde_json::json!("healthy")
    );

    // record_event + delete.
    reg.record_event(
        ID,
        "test",
        Some("tester"),
        serde_json::json!({ "ok": true }),
    )
    .await
    .unwrap();
    reg.delete(ID).await.unwrap();
    assert!(reg.get(ID).await.is_err(), "deleted project must be gone");

    // Cleanup the audit rows for this id (delete() doesn't cascade).
    sqlx::query("DELETE FROM project_events WHERE project_id = $1")
        .bind(ID)
        .execute(&pool)
        .await
        .ok();
}
