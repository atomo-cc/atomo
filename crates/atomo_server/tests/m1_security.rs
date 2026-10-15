//! Real PostgreSQL plus the production HTTP routers. Run serially in tests/m1.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

const SCHEMA: &str = r#"
import { model, text, number, relation, action, allow } from '@atomo-cc/schema';
export const Parent = model('m1_parents', {
 fields: { id: text().id(), ownerId: text().fromAuth('userId'), tenantId: text().fromAuth('tenantId'), title: text().required(), quota: number().required().min(0) },
 access: {
  create: allow.authenticated(),
  read: allow.any([allow.owner('ownerId'), allow.role('admin')]),
  update: allow.all([allow.authenticated(), allow.any([allow.owner('ownerId'), allow.role('admin')])]),
  delete: allow.owner('ownerId'),
 },
});
export const Child = model('m1_children', {
 fields: { id: text().id(), parentId: relation('m1_parents').required(), title: text().required() },
 access: { create: allow.authenticated(), read: allow.authenticated(), update: allow.authenticated() },
});
export const Global = model('m1_globals', {
 tenant: 'global',
 fields: { id: text().id(), title: text().required() },
 access: { create: allow.role('admin'), read: allow.authenticated() },
});
export const GlobalChild = model('m1_global_children', {
 fields: { id: text().id(), parentId: relation('m1_globals').required() },
 access: { create: allow.authenticated(), read: allow.authenticated() },
});
export const Owned = model('m1_owned', {
 fields: { id: text().id(), ownerId: text().required(), title: text().required() },
 access: { create: allow.authenticated(), read: allow.owner('ownerId'), update: allow.owner('ownerId'), delete: allow.owner('ownerId') },
});
export const inspect = action('inspect').input({ recordId: text().required(), quota: number().required() }).returns('m1_parents');
export const restricted = action('restricted').input({ note: text().required() }).returns('m1_globals');
"#;

async fn http(
    app: &Router,
    path: &str,
    method: &str,
    token: Option<&str>,
    tenant: Option<&str>,
    body: Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .uri(path)
        .method(method)
        .header("content-type", "application/json");
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    if let Some(tenant) = tenant {
        req = req.header("x-tenant-id", tenant);
    }
    if let Some(key) = key {
        req = req.header("idempotency-key", key);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1_000_000)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn gql(app: &Router, token: &str, tenant: Option<&str>, query: String) -> Value {
    let (status, data) = http(
        app,
        "/graphql",
        "POST",
        Some(token),
        tenant,
        json!({"query": query}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    data
}

async fn create(app: &Router, token: &str, model: &str, fields: &str) -> Value {
    gql(
        app,
        token,
        None,
        format!("mutation {{ create(model: \"{model}\", data: {{{fields}}}) }}"),
    )
    .await
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL; tests/m1/compose.yaml"]
async fn m1_cross_entry_security() {
    let url = std::env::var("DATABASE_URL").unwrap();
    let setup = sqlx::PgPool::connect(&url).await.unwrap();
    for table in [
        "m1_owned",
        "m1_global_children",
        "m1_children",
        "m1_parents",
        "m1_globals",
    ] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
            .execute(&setup)
            .await
            .unwrap();
    }
    let atomo = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(url)
        .enable_migrations(true)
        .build()
        .await
        .unwrap();
    let pool = atomo.db_pool().clone();
    // Only this fixture's tables are cleaned; no shared consumer database is used.
    for table in [
        "m1_owned",
        "m1_global_children",
        "m1_children",
        "m1_parents",
        "m1_globals",
    ] {
        sqlx::query(&format!("TRUNCATE {table} CASCADE"))
            .execute(&pool)
            .await
            .unwrap();
    }
    atomo_server::ensure_platform_tables(&pool).await.unwrap();
    let auth =
        atomo_server::auth::HttpAuthService::new(&uuid::Uuid::new_v4().to_string(), pool.clone());
    let mut users = Vec::new();
    for (role, tenant) in [
        ("viewer", Some("a")),
        ("viewer", Some("a")),
        ("viewer", Some("b")),
        ("admin", None),
        ("viewer", None),
    ] {
        let id = atomo_core::EntityId::new().to_string();
        let email = format!("{id}@example.invalid");
        sqlx::query("INSERT INTO users (id,email,password_hash,first_name,last_name,role,tenant_id) VALUES ($1,$2,'unused','Test','Principal',$3,$4)").bind(&id).bind(&email).bind(role).bind(tenant).execute(&pool).await.unwrap();
        let (token, _) = auth.issue_tokens(&id, &email, role).await.unwrap();
        users.push((id, token));
    }
    let jobs = Arc::new(atomo_server::jobs::JobStore::new(
        pool.clone(),
        atomo.event_sender(),
    ));
    jobs.init().await.unwrap();
    let workers = Arc::new(atomo_server::jobs::WorkerTokenStore::new(pool.clone()));
    workers.init().await.unwrap();
    let (_, worker_token) = workers
        .mint("m1", &["actions".into()], &["crud:*".into()])
        .await
        .unwrap();
    let redirects = Arc::new(atomo_server::public_read_redirects::RedirectStore::new(
        pool.clone(),
    ));
    redirects.init().await.unwrap();
    let schema = atomo.schema().clone();
    let client = Arc::new(atomo.client().clone());
    let actions =
        atomo_server::action_routes::action_router(schema.clone(), jobs.clone(), auth.clone());
    let crud = atomo_server::crud_routes::crud_router(client.clone(), schema.clone(), workers);
    let app = atomo_server::handlers::create_router(
        atomo_server::handlers::build_extended_schema(&atomo),
        atomo,
        auth.clone(),
        atomo_server::audit::HttpAuditService::new(pool.clone()),
        atomo_server::auth::RegistrationConfig::disabled(),
        vec![],
        redirects,
    )
    .merge(actions)
    .merge(crud);

    let a = &users[0].1;
    let peer = &users[1].1;
    let b = &users[2].1;
    let admin = &users[3].1;
    let created = create(&app, a, "Parent", "title: \"owned\", quota: 10").await;
    assert!(
        created.get("errors").is_none(),
        "valid sourced create rejected"
    );
    let id = created["data"]["create"]["id"].as_str().unwrap();
    assert_eq!(created["data"]["create"]["tenantId"], "a");
    assert_eq!(created["data"]["create"]["ownerId"], users[0].0);
    let owned = create(
        &app,
        peer,
        "Owned",
        &format!("title:\"raw owner\",ownerId:\"{}\"", users[0].0),
    )
    .await;
    let owned_id = owned["data"]["create"]["id"].as_str().unwrap();
    let takeover = gql(
        &app,
        peer,
        None,
        format!(
            "mutation {{ update(model:\"Owned\",id:\"{owned_id}\",data:{{ownerId:\"{}\"}}) }}",
            users[1].0
        ),
    )
    .await;
    assert!(
        takeover["data"]["update"].is_null(),
        "new owner must not authorize an old row"
    );
    let transfer = gql(
        &app,
        a,
        None,
        format!(
            "mutation {{ update(model:\"Owned\",id:\"{owned_id}\",data:{{ownerId:\"{}\"}}) }}",
            users[1].0
        ),
    )
    .await;
    assert!(
        transfer.get("errors").is_some(),
        "post-update owner policy must also hold"
    );
    let owner_edit = gql(
        &app,
        a,
        None,
        format!(
            "mutation {{ update(model:\"Owned\",id:\"{owned_id}\",data:{{title:\"allowed\"}}) }}"
        ),
    )
    .await;
    assert_eq!(owner_edit["data"]["update"]["title"], "allowed");
    for token in [peer, b] {
        let list = gql(
            &app,
            token,
            None,
            "{ records(model: \"Parent\", limit: 1) }".into(),
        )
        .await;
        assert_eq!(
            list["data"]["records"].as_array().unwrap().len(),
            0,
            "owner read leak"
        );
        let result = gql(
            &app,
            token,
            None,
            format!(
                "mutation {{ update(model:\"Parent\",id:\"{id}\",data:{{title:\"stolen\"}}) }}"
            ),
        )
        .await;
        assert!(result["data"]["update"].is_null());
    }
    let (wrong, _) = http(
        &app,
        "/graphql",
        "POST",
        Some(a),
        Some("b"),
        json!({"query":"{ records(model: \"Parent\") }"}),
        None,
    )
    .await;
    assert_eq!(wrong, StatusCode::FORBIDDEN);
    let missing = create(&app, &users[4].1, "Parent", "title:\"unbound\",quota:1").await;
    assert!(missing.get("errors").is_some());
    for fields in [
        format!("title:\"forged\",quota:1,ownerId:\"{}\"", users[1].0),
        "title:\"fraction\",quota:99.5".into(),
        "title:\"overflow\",quota:9223372036854775808".into(),
        "title:\"unsafe\",quota:9007199254740992".into(),
        "title:\"negative\",quota:-1".into(),
    ] {
        assert!(create(&app, a, "Parent", &fields)
            .await
            .get("errors")
            .is_some());
    }
    let change_owner = gql(
        &app,
        a,
        None,
        format!(
            "mutation {{ update(model:\"Parent\",id:\"{id}\",data:{{ownerId:\"{}\"}}) }}",
            users[1].0
        ),
    )
    .await;
    assert!(change_owner.get("errors").is_some());
    let child = create(
        &app,
        a,
        "Child",
        &format!("parentId:\"{id}\",title:\"valid\""),
    )
    .await;
    assert!(child.get("errors").is_none());
    for token in [peer, b] {
        assert!(create(
            &app,
            token,
            "Child",
            &format!("parentId:\"{id}\",title:\"foreign\"")
        )
        .await
        .get("errors")
        .is_some());
    }
    let own_b = create(&app, b, "Parent", "title:\"b\",quota:1").await;
    let child_id = child["data"]["create"]["id"].as_str().unwrap();
    let rebind = gql(
        &app,
        a,
        None,
        format!(
            "mutation {{ update(model:\"Child\",id:\"{child_id}\",data:{{parentId:\"{}\"}}) }}",
            own_b["data"]["create"]["id"].as_str().unwrap()
        ),
    )
    .await;
    assert!(rebind.get("errors").is_some());
    let principal = atomo::security::Principal {
        user_id: Some(users[0].0.clone()),
        role: Some("viewer".into()),
        tenant_id: Some("a".into()),
    };
    let before = atomo::security::with_principal(principal.clone(), client.count("Child", &[]))
        .await
        .unwrap();
    let bulk = vec![
        std::collections::HashMap::from([
            ("parentId".into(), json!(id)),
            ("title".into(), json!("bulk valid")),
        ]),
        std::collections::HashMap::from([
            ("parentId".into(), own_b["data"]["create"]["id"].clone()),
            ("title".into(), json!("bulk invalid")),
        ]),
    ];
    assert!(atomo::security::with_principal(
        principal.clone(),
        client.create_many("Child", &bulk, None)
    )
    .await
    .is_err());
    assert_eq!(
        atomo::security::with_principal(principal.clone(), client.count("Child", &[]))
            .await
            .unwrap(),
        before,
        "bulk must roll back all rows"
    );
    let global = create(&app, admin, "Global", "title:\"public reference\"").await;
    assert!(global.get("errors").is_none());
    let global_id = global["data"]["create"]["id"].as_str().unwrap();
    assert!(
        create(&app, a, "GlobalChild", &format!("parentId:\"{global_id}\""))
            .await
            .get("errors")
            .is_none()
    );

    let delegated_id = jobs
        .enqueue_as(
            "actions",
            "m1-worker",
            json!({"actor":users[0].0}),
            None,
            5,
            0,
            Some("a"),
            Some(&users[0].0),
        )
        .await
        .unwrap();
    let leased = jobs.lease(&["actions".into()], 200, 300).await.unwrap();
    let delegated = leased.iter().find(|j| j.id == delegated_id).unwrap();
    let arbitrary_id = jobs
        .enqueue(
            "actions",
            "untrusted-payload",
            json!({"actor": users[3].0}),
            None,
            5,
            0,
            Some("a"),
        )
        .await
        .unwrap();
    let arbitrary = jobs.lease(&["actions".into()], 200, 300).await.unwrap();
    let arbitrary = arbitrary.iter().find(|j| j.id == arbitrary_id).unwrap();
    for actor in [
        json!({"jobId":arbitrary.id,"leaseId":arbitrary.lease_id}),
        json!({"jobId":delegated.id,"leaseId":"invalid"}),
        json!({"jobId":delegated.id,"leaseId":delegated.lease_id,"userId":users[3].0}),
    ] {
        let request = Request::builder()
            .uri("/api/worker/crud/Parent")
            .method("POST")
            .header("x-worker-token", &worker_token)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"actor":actor,"data":{"title":"impersonation","quota":1}}).to_string(),
            ))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    // Worker capability permits delegation, but actor role/tenant are reloaded.
    for (actor, data, expected) in [
        (
            json!({"userId":users[0].0}),
            json!({"title":"worker valid","quota":2}),
            StatusCode::CREATED,
        ),
        (
            json!({"userId":users[0].0,"role":"admin"}),
            json!({"title":"forged","quota":2}),
            StatusCode::FORBIDDEN,
        ),
        (
            json!({"userId":users[0].0,"tenantId":"b"}),
            json!({"title":"wrong","quota":2}),
            StatusCode::FORBIDDEN,
        ),
        (
            json!({"userId":users[0].0}),
            json!({"title":"fraction","quota":99.5}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let mut actor = actor;
        actor["jobId"] = json!(delegated.id);
        actor["leaseId"] = json!(delegated.lease_id);
        let req = Request::builder()
            .uri("/api/worker/crud/Parent")
            .method("POST")
            .header("x-worker-token", &worker_token)
            .header("content-type", "application/json")
            .body(Body::from(json!({"actor":actor,"data":data}).to_string()))
            .unwrap();
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(), expected);
    }
    let queued_before: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    for (path, token, data, expected) in [
        (
            "inspect",
            None,
            json!({"recordId":id,"quota":1}),
            StatusCode::UNAUTHORIZED,
        ),
        (
            "inspect",
            Some(a.as_str()),
            json!({"quota":1}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "inspect",
            Some(a.as_str()),
            json!({"recordId":id,"quota":1.5}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "restricted",
            Some(a.as_str()),
            json!({"note":"denied"}),
            StatusCode::FORBIDDEN,
        ),
    ] {
        assert_eq!(
            http(
                &app,
                &format!("/api/actions/{path}"),
                "POST",
                token,
                None,
                data,
                None
            )
            .await
            .0,
            expected
        );
    }
    assert_eq!(
        http(
            &app,
            "/api/actions/inspect",
            "POST",
            Some(a),
            Some("b"),
            json!({"recordId":id,"quota":1}),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let queued_after: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        queued_before, queued_after,
        "rejected action requests must not enqueue"
    );
    let data = json!({"recordId":id,"quota":1});
    let first = http(
        &app,
        "/api/actions/inspect",
        "POST",
        Some(a),
        None,
        data.clone(),
        Some("retry"),
    )
    .await;
    assert_eq!(first.0, StatusCode::OK);
    let second = http(
        &app,
        "/api/actions/inspect",
        "POST",
        Some(a),
        None,
        data,
        Some("retry"),
    )
    .await;
    assert_eq!(first.1["jobId"], second.1["jobId"]);
    assert_eq!(
        http(
            &app,
            "/api/actions/inspect",
            "POST",
            Some(a),
            None,
            json!({"recordId":id,"quota":2}),
            Some("retry")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let tenant: Option<String> = sqlx::query_scalar("SELECT tenant_id FROM jobs WHERE id=$1")
        .bind(first.1["jobId"].as_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tenant.as_deref(), Some("a"));

    // Test generated RLS with a non-bypass role, retaining legacy null rows.
    atomo_server::rls::check_connection_role(&pool, false)
        .await
        .unwrap();
    let legacy = client
        .create(
            "Child",
            &std::collections::HashMap::from([
                ("title".into(), json!("legacy null")),
                ("parentId".into(), json!(id)),
            ]),
            &[],
            None,
        )
        .await
        .unwrap();
    atomo_server::rls::ensure_schema_rls(&pool, &schema)
        .await
        .unwrap();
    atomo_server::rls::ensure_schema_rls(&pool, &schema)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM m1_children WHERE id=$1")
        .bind(legacy["id"].as_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "legacy null row must be quarantined by RLS");
    for token in [a, b] {
        let list = gql(&app, token, None, "{ records(model:\"Parent\") }".into()).await;
        assert!(list.get("errors").is_none());
        assert!(!list["data"]["records"].as_array().unwrap().is_empty());
    }
}
