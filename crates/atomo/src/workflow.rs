//! Workflow engine: define and execute multi-step workflows with conditions

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

/// A workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub trigger: WorkflowTrigger,
    pub steps: Vec<WorkflowStep>,
}

/// What triggers the workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowTrigger {
    /// Triggered by a model event
    OnEvent { model: String, event_type: String },
    /// Triggered manually via API
    Manual,
    /// Triggered on a schedule (cron expression)
    Schedule { cron: String },
}

/// A single step in a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub action: StepAction,
    pub condition: Option<Condition>,
    pub on_failure: FailurePolicy,
}

/// Actions a step can perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepAction {
    /// Execute a GraphQL mutation
    Mutation {
        query: String,
        variables: HashMap<String, Value>,
    },
    /// Call a WASM plugin function
    Plugin {
        plugin_name: String,
        function: String,
    },
    /// Enqueue a durable job for an external worker (the job id is stored in `context["job_id"]`).
    Job {
        queue: String,
        kind: String,
        #[serde(default)]
        payload: Value,
        #[serde(default)]
        idempotency_key: Option<String>,
    },
    /// Send an HTTP request
    Http {
        method: String,
        url: String,
        body: Option<Value>,
    },
    /// Wait for a duration
    Delay { seconds: u64 },
    /// Set a variable in the workflow context
    SetVariable { key: String, value: Value },
}

/// Condition for step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: String, // "eq", "neq", "gt", "lt", "contains"
    pub value: Value,
}

/// What to do when a step fails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailurePolicy {
    Stop,
    Continue,
    Retry { max_attempts: u32 },
}

/// Runtime state of a workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowExecution {
    pub workflow_name: String,
    pub status: ExecutionStatus,
    pub current_step: usize,
    pub context: HashMap<String, Value>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Paused,
}

/// Workflow engine that manages definitions and executions
pub struct WorkflowEngine {
    workflows: std::sync::RwLock<HashMap<String, Workflow>>,
    pool: Option<sqlx::PgPool>,
    /// Executes a workflow `Mutation` step's GraphQL query. Implemented by the server (which
    /// holds the GraphQL schema) and injected here, so the engine stays decoupled from the
    /// GraphQL/server layer (no dependency cycle).
    mutation_executor: Option<Arc<dyn MutationExecutor>>,
    plugin_executor: Option<Arc<dyn PluginExecutor>>,
    job_executor: Option<Arc<dyn JobExecutor>>,
}

/// Seam for executing a workflow `Job` step. The server implements it against the durable job
/// store, so a no-code workflow can enqueue work for an external worker.
#[async_trait::async_trait]
pub trait JobExecutor: Send + Sync {
    /// Enqueue a job on `queue` with `kind`/`payload`; returns the job id.
    async fn enqueue(
        &self,
        queue: &str,
        kind: &str,
        payload: &Value,
        idempotency_key: Option<&str>,
    ) -> Result<String>;
}

/// Seam for executing a workflow `Mutation` step. The engine defines it; the server implements
/// it against the built GraphQL schema.
#[async_trait::async_trait]
pub trait MutationExecutor: Send + Sync {
    /// Run a GraphQL mutation `query` with `variables`; return Ok on success.
    async fn execute(&self, query: &str, variables: &HashMap<String, Value>) -> Result<()>;
}

/// Seam for executing a workflow `Plugin` step. The server implements it against the
/// WasmPluginManager (which handles both WASM + Javy/JS plugins via `call_hook`).
#[async_trait::async_trait]
pub trait PluginExecutor: Send + Sync {
    /// Call `function` on `plugin_name`, passing the workflow context as JSON; returns the output
    /// (or None if the plugin returned nothing/identity).
    async fn execute(
        &self,
        plugin_name: &str,
        function: &str,
        context_json: &str,
    ) -> Result<Option<String>>;
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self {
            workflows: std::sync::RwLock::new(HashMap::new()),
            pool: None,
            mutation_executor: None,
            plugin_executor: None,
            job_executor: None,
        }
    }

    /// Create an engine backed by a Postgres table for durable persistence.
    pub fn with_pool(pool: sqlx::PgPool) -> Self {
        Self {
            workflows: std::sync::RwLock::new(HashMap::new()),
            pool: Some(pool),
            mutation_executor: None,
            plugin_executor: None,
            job_executor: None,
        }
    }

    /// Inject the executor that runs `Mutation` steps (set at server boot).
    pub fn set_mutation_executor(&mut self, exec: Arc<dyn MutationExecutor>) {
        self.mutation_executor = Some(exec);
    }

    /// Inject the executor that runs `Plugin` steps (set at server boot).
    pub fn set_plugin_executor(&mut self, exec: Arc<dyn PluginExecutor>) {
        self.plugin_executor = Some(exec);
    }

    /// Inject the executor that runs `Job` steps (set at server boot).
    pub fn set_job_executor(&mut self, exec: Arc<dyn JobExecutor>) {
        self.job_executor = Some(exec);
    }

    /// Ensure the workflows table exists and load any persisted definitions into memory.
    pub async fn init(&self) -> Result<()> {
        let pool = match &self.pool {
            Some(p) => p,
            None => return Ok(()),
        };
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS workflows (
                name TEXT PRIMARY KEY,
                definition JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(pool)
        .await?;
        let rows = sqlx::query_as::<_, (Value,)>("SELECT definition FROM workflows")
            .fetch_all(pool)
            .await?;
        let mut map = self.workflows.write().unwrap();
        for (def,) in rows {
            if let Ok(wf) = serde_json::from_value::<Workflow>(def) {
                map.insert(wf.name.clone(), wf);
            }
        }
        Ok(())
    }

    /// Persist a workflow definition (upsert). No-op without a pool.
    pub async fn persist(&self, workflow: &Workflow) -> Result<()> {
        if let Some(pool) = &self.pool {
            sqlx::query(
                "INSERT INTO workflows (name, definition) VALUES ($1, $2)
                 ON CONFLICT (name) DO UPDATE SET definition = $2, updated_at = NOW()",
            )
            .bind(&workflow.name)
            .bind(serde_json::to_value(workflow)?)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    /// Delete a persisted workflow. No-op without a pool.
    pub async fn unpersist(&self, name: &str) -> Result<()> {
        if let Some(pool) = &self.pool {
            sqlx::query("DELETE FROM workflows WHERE name = $1")
                .bind(name)
                .execute(pool)
                .await?;
        }
        Ok(())
    }

    pub fn register(&self, workflow: Workflow) {
        self.workflows
            .write()
            .unwrap()
            .insert(workflow.name.clone(), workflow);
    }

    /// Remove a workflow by name. Returns true if it existed.
    pub fn remove(&self, name: &str) -> bool {
        self.workflows.write().unwrap().remove(name).is_some()
    }

    /// List registered workflow names
    pub fn list(&self) -> Vec<String> {
        self.workflows.read().unwrap().keys().cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<Workflow> {
        self.workflows.read().unwrap().get(name).cloned()
    }

    /// Execute a workflow with initial context
    pub async fn execute(
        &self,
        name: &str,
        initial_context: HashMap<String, Value>,
    ) -> Result<WorkflowExecution> {
        let workflow = self
            .workflows
            .read()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Workflow '{}' not found", name))?;

        let mut execution = WorkflowExecution {
            workflow_name: name.to_string(),
            status: ExecutionStatus::Running,
            current_step: 0,
            context: initial_context,
            errors: Vec::new(),
        };

        for (i, step) in workflow.steps.iter().enumerate() {
            execution.current_step = i;

            // Check condition
            if let Some(cond) = &step.condition {
                if !evaluate_condition(cond, &execution.context) {
                    continue;
                }
            }

            // Execute step
            match execute_step(
                &step.action,
                &mut execution.context,
                self.mutation_executor.as_ref(),
                self.plugin_executor.as_ref(),
                self.job_executor.as_ref(),
            )
            .await
            {
                Ok(()) => {}
                Err(e) => {
                    execution
                        .errors
                        .push(format!("Step '{}': {}", step.name, e));
                    match &step.on_failure {
                        FailurePolicy::Stop => {
                            execution.status = ExecutionStatus::Failed;
                            return Ok(execution);
                        }
                        FailurePolicy::Continue => continue,
                        FailurePolicy::Retry { max_attempts } => {
                            let mut success = false;
                            for _ in 0..*max_attempts {
                                if execute_step(
                                    &step.action,
                                    &mut execution.context,
                                    self.mutation_executor.as_ref(),
                                    self.plugin_executor.as_ref(),
                                    self.job_executor.as_ref(),
                                )
                                .await
                                .is_ok()
                                {
                                    success = true;
                                    break;
                                }
                            }
                            if !success {
                                execution.status = ExecutionStatus::Failed;
                                return Ok(execution);
                            }
                        }
                    }
                }
            }
        }

        execution.status = ExecutionStatus::Completed;
        Ok(execution)
    }

    /// Find workflows triggered by a specific event
    pub fn find_by_trigger(&self, model: &str, event_type: &str) -> Vec<Workflow> {
        self.workflows.read().unwrap().values()
            .filter(|w| matches!(&w.trigger, WorkflowTrigger::OnEvent { model: m, event_type: e } if m == model && e == event_type))
            .cloned()
            .collect()
    }

    /// Start listening to model events and auto-trigger matching workflows
    pub fn start_event_listener(
        self: Arc<Self>,
        mut rx: tokio::sync::broadcast::Receiver<crate::events::ModelEvent>,
    ) {
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let event_type_str = format!("{:?}", event.event_type);
                        let workflows = self.find_by_trigger(&event.model_name, &event_type_str);
                        for workflow in workflows {
                            let ctx = event.data.clone();
                            if let Err(e) = self.execute(&workflow.name, ctx).await {
                                tracing::error!(workflow = %workflow.name, error = %e, "Workflow execution failed");
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Workflow listener lagged by {} events", n);
                    }
                    Err(_) => break,
                }
            }
        });
    }

    /// Start a background task that fires Schedule-triggered workflows on their cron cadence.
    pub fn start_scheduler(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut last_tick = chrono::Utc::now();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                let now = chrono::Utc::now();
                let scheduled: Vec<(String, String)> = self
                    .workflows
                    .read()
                    .unwrap()
                    .values()
                    .filter_map(|w| match &w.trigger {
                        WorkflowTrigger::Schedule { cron } => Some((w.name.clone(), cron.clone())),
                        _ => None,
                    })
                    .collect();
                for (name, cron_expr) in scheduled {
                    match cron_should_fire(&cron_expr, last_tick, now) {
                        Ok(true) => {
                            if let Err(e) = self.execute(&name, HashMap::new()).await {
                                tracing::error!(workflow = %name, error = %e, "Scheduled workflow failed");
                            }
                        }
                        Ok(false) => {}
                        Err(e) => {
                            tracing::warn!(workflow = %name, error = %e, "Invalid cron expression")
                        }
                    }
                }
                last_tick = now;
            }
        });
    }
}

/// Pure helper: does `cron_expr` have a scheduled occurrence in the window `(last_tick, now]`?
/// Returns Err with a message if the cron expression is invalid.
pub fn cron_should_fire(
    cron_expr: &str,
    last_tick: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<bool, String> {
    let schedule = cron::Schedule::from_str(cron_expr).map_err(|e| e.to_string())?;
    Ok(schedule
        .after(&last_tick)
        .next()
        .map(|next| next <= now)
        .unwrap_or(false))
}

fn evaluate_condition(cond: &Condition, context: &HashMap<String, Value>) -> bool {
    let val = context.get(&cond.field).unwrap_or(&Value::Null);
    match cond.operator.as_str() {
        "eq" => val == &cond.value,
        "neq" => val != &cond.value,
        "gt" => val.as_f64().unwrap_or(0.0) > cond.value.as_f64().unwrap_or(0.0),
        "lt" => val.as_f64().unwrap_or(0.0) < cond.value.as_f64().unwrap_or(0.0),
        "contains" => val
            .as_str()
            .unwrap_or("")
            .contains(cond.value.as_str().unwrap_or("")),
        _ => true,
    }
}

async fn execute_step(
    action: &StepAction,
    context: &mut HashMap<String, Value>,
    mutation_executor: Option<&Arc<dyn MutationExecutor>>,
    plugin_executor: Option<&Arc<dyn PluginExecutor>>,
    job_executor: Option<&Arc<dyn JobExecutor>>,
) -> Result<()> {
    match action {
        StepAction::Delay { seconds } => {
            tokio::time::sleep(std::time::Duration::from_secs(*seconds)).await;
        }
        StepAction::SetVariable { key, value } => {
            context.insert(key.clone(), value.clone());
        }
        StepAction::Http { method, url, body } => {
            // Actually perform the request (was previously a no-op log). Templating of url/body
            // from context is a future enhancement; this executes the literal request.
            let client = reqwest::Client::new();
            let m =
                reqwest::Method::from_str(&method.to_uppercase()).unwrap_or(reqwest::Method::POST);
            let mut req = client.request(m, url);
            if let Some(b) = body {
                req = req.json(b);
            }
            let resp = req.send().await?;
            let status = resp.status();
            context.insert(
                "http_status".to_string(),
                Value::Number(status.as_u16().into()),
            );
            if !status.is_success() {
                anyhow::bail!("HTTP step to {} failed with status {}", url, status);
            }
        }
        StepAction::Mutation { query, variables } => {
            // Run via the injected executor (server wires it against the GraphQL schema). If no
            // executor is configured, fail loudly rather than silently passing.
            match mutation_executor {
                Some(exec) => exec.execute(query, variables).await?,
                None => anyhow::bail!(
                    "workflow Mutation step: no executor configured (server must inject one)"
                ),
            }
        }
        StepAction::Plugin {
            plugin_name,
            function,
        } => match plugin_executor {
            Some(exec) => {
                let ctx_json = serde_json::to_string(&*context).unwrap_or_default();
                let output = exec.execute(plugin_name, function, &ctx_json).await?;
                if let Some(out) = output {
                    context.insert(
                        "plugin_output".to_string(),
                        serde_json::from_str(&out).unwrap_or(Value::String(out)),
                    );
                }
            }
            None => anyhow::bail!(
                "workflow Plugin step: no executor configured (server must inject one)"
            ),
        },
        StepAction::Job {
            queue,
            kind,
            payload,
            idempotency_key,
        } => match job_executor {
            Some(exec) => {
                let id = exec
                    .enqueue(queue, kind, payload, idempotency_key.as_deref())
                    .await?;
                context.insert("job_id".to_string(), Value::String(id));
            }
            None => {
                anyhow::bail!("workflow Job step: no executor configured (server must inject one)")
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    // B1: a workflow triggered by a Deal stage change is found by the event listener path.
    #[test]
    fn deal_update_event_finds_workflow() {
        let engine = WorkflowEngine::new();
        engine.register(Workflow {
            name: "sales-pipeline".to_string(),
            trigger: WorkflowTrigger::OnEvent {
                model: "Deal".to_string(),
                event_type: "Updated".to_string(),
            },
            steps: vec![],
        });
        // Mirrors workflow.rs event listener: find_by_trigger(model, format!("{:?}", event_type)).
        let found = engine.find_by_trigger("Deal", "Updated");
        assert_eq!(
            found.len(),
            1,
            "Deal/Updated event must find the sales-pipeline workflow"
        );
        assert!(
            engine.find_by_trigger("Contact", "Updated").is_empty(),
            "must not match other models"
        );
    }

    // B1: the Http step actually performs a request (was a no-op log before).
    #[tokio::test]
    async fn http_step_actually_sends_request() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        // One-shot local HTTP server that records it was hit and returns 200.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await.unwrap();
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            true // hit
        });

        let engine = WorkflowEngine::new();
        engine.register(Workflow {
            name: "wh".to_string(),
            trigger: WorkflowTrigger::Manual,
            steps: vec![WorkflowStep {
                name: "notify".to_string(),
                action: StepAction::Http {
                    method: "POST".to_string(),
                    url: format!("http://{}/hook", addr),
                    body: Some(json!({"deal": "x"})),
                },
                condition: None,
                on_failure: FailurePolicy::Stop,
            }],
        });

        let exec = engine.execute("wh", HashMap::new()).await.unwrap();
        assert_eq!(
            exec.status,
            ExecutionStatus::Completed,
            "errors: {:?}",
            exec.errors
        );
        assert_eq!(
            exec.context.get("http_status").and_then(|v| v.as_u64()),
            Some(200)
        );
        assert!(
            server.await.unwrap(),
            "the HTTP server should have been hit"
        );
    }

    // Item 4/3: the Mutation step with NO executor configured must FAIL the workflow.
    #[tokio::test]
    async fn mutation_step_fails_loudly() {
        let engine = WorkflowEngine::new();
        engine.register(Workflow {
            name: "m".to_string(),
            trigger: WorkflowTrigger::Manual,
            steps: vec![WorkflowStep {
                name: "mut".to_string(),
                action: StepAction::Mutation {
                    query: "mutation { noop }".into(),
                    variables: HashMap::new(),
                },
                condition: None,
                on_failure: FailurePolicy::Stop,
            }],
        });
        let exec = engine.execute("m", HashMap::new()).await.unwrap();
        assert_eq!(
            exec.status,
            ExecutionStatus::Failed,
            "Mutation step must fail, not silently pass"
        );
        assert!(
            exec.errors.iter().any(|e| e.contains("no executor")),
            "errors: {:?}",
            exec.errors
        );
    }

    // Item 3: with an executor injected, the Mutation step RUNS it (and passes the query through).
    #[tokio::test]
    async fn mutation_step_runs_via_injected_executor() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct RecordingExecutor(std::sync::Arc<AtomicBool>);
        #[async_trait::async_trait]
        impl MutationExecutor for RecordingExecutor {
            async fn execute(&self, query: &str, _vars: &HashMap<String, Value>) -> Result<()> {
                assert!(
                    query.contains("create"),
                    "executor receives the step's query"
                );
                self.0.store(true, Ordering::SeqCst);
                Ok(())
            }
        }
        let hit = std::sync::Arc::new(AtomicBool::new(false));
        let mut engine = WorkflowEngine::new();
        engine.set_mutation_executor(std::sync::Arc::new(RecordingExecutor(hit.clone())));
        engine.register(Workflow {
            name: "m2".to_string(),
            trigger: WorkflowTrigger::Manual,
            steps: vec![WorkflowStep {
                name: "mut".to_string(),
                action: StepAction::Mutation {
                    query: "mutation { create(model: \"X\") }".into(),
                    variables: HashMap::new(),
                },
                condition: None,
                on_failure: FailurePolicy::Stop,
            }],
        });
        let exec = engine.execute("m2", HashMap::new()).await.unwrap();
        assert_eq!(
            exec.status,
            ExecutionStatus::Completed,
            "errors: {:?}",
            exec.errors
        );
        assert!(
            hit.load(Ordering::SeqCst),
            "the injected executor must have run"
        );
    }

    #[test]
    fn cron_fires_within_window() {
        let last_tick = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let now = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 30).unwrap();
        assert!(cron_should_fire("0 * * * * *", last_tick, now).unwrap());
    }

    #[test]
    fn cron_does_not_fire_outside_window() {
        let last_tick = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();
        let now = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 30).unwrap();
        assert!(!cron_should_fire("0 * * * * *", last_tick, now).unwrap());
    }

    #[test]
    fn cron_invalid_returns_err() {
        let t = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        assert!(cron_should_fire("not-a-cron", t, t).is_err());
    }

    #[test]
    fn register_and_list() {
        let engine = WorkflowEngine::new();
        engine.register(Workflow {
            name: "w1".into(),
            trigger: WorkflowTrigger::Manual,
            steps: vec![],
        });
        assert!(engine.list().contains(&"w1".to_string()));
        assert!(engine.get("w1").is_some());
    }

    #[test]
    fn find_by_trigger_matches_event() {
        let engine = WorkflowEngine::new();
        engine.register(Workflow {
            name: "on_contact".into(),
            trigger: WorkflowTrigger::OnEvent {
                model: "Contact".into(),
                event_type: "Created".into(),
            },
            steps: vec![],
        });
        assert_eq!(engine.find_by_trigger("Contact", "Created").len(), 1);
        assert!(engine.find_by_trigger("Contact", "Updated").is_empty());
    }

    #[tokio::test]
    async fn execute_runs_steps() {
        let engine = WorkflowEngine::new();
        engine.register(Workflow {
            name: "test_wf".into(),
            trigger: WorkflowTrigger::Manual,
            steps: vec![WorkflowStep {
                name: "set_done".into(),
                action: StepAction::SetVariable {
                    key: "done".into(),
                    value: json!(true),
                },
                condition: None,
                on_failure: FailurePolicy::Continue,
            }],
        });
        let exec = engine.execute("test_wf", HashMap::new()).await.unwrap();
        assert_eq!(exec.status, ExecutionStatus::Completed);
        assert_eq!(exec.context.get("done"), Some(&json!(true)));
    }

    #[tokio::test]
    async fn plugin_step_runs_via_injected_executor() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct MockPlugin(std::sync::Arc<AtomicBool>);
        #[async_trait::async_trait]
        impl PluginExecutor for MockPlugin {
            async fn execute(&self, name: &str, func: &str, _ctx: &str) -> Result<Option<String>> {
                assert_eq!(name, "normalize-contact");
                assert_eq!(func, "on_create");
                self.0.store(true, Ordering::SeqCst);
                Ok(Some(r#"{"normalized":true}"#.to_string()))
            }
        }
        let hit = std::sync::Arc::new(AtomicBool::new(false));
        let mut engine = WorkflowEngine::new();
        engine.set_plugin_executor(std::sync::Arc::new(MockPlugin(hit.clone())));
        engine.register(Workflow {
            name: "plugin_wf".to_string(),
            trigger: WorkflowTrigger::Manual,
            steps: vec![WorkflowStep {
                name: "plug".to_string(),
                action: StepAction::Plugin {
                    plugin_name: "normalize-contact".into(),
                    function: "on_create".into(),
                },
                condition: None,
                on_failure: FailurePolicy::Stop,
            }],
        });
        let exec = engine.execute("plugin_wf", HashMap::new()).await.unwrap();
        assert_eq!(
            exec.status,
            ExecutionStatus::Completed,
            "errors: {:?}",
            exec.errors
        );
        assert!(hit.load(Ordering::SeqCst), "plugin executor must have run");
        assert_eq!(
            exec.context.get("plugin_output"),
            Some(&json!({"normalized": true})),
            "plugin output stored in context"
        );
    }

    #[tokio::test]
    async fn job_step_enqueues_via_injected_executor() {
        // Records the enqueue args (queue, kind, payload, key) and returns a fixed job id.
        type Recorded = (String, String, Value, Option<String>);
        struct MockJobs(std::sync::Mutex<Option<Recorded>>);
        #[async_trait::async_trait]
        impl JobExecutor for MockJobs {
            async fn enqueue(
                &self,
                queue: &str,
                kind: &str,
                payload: &Value,
                idempotency_key: Option<&str>,
            ) -> Result<String> {
                *self.0.lock().unwrap() = Some((
                    queue.to_string(),
                    kind.to_string(),
                    payload.clone(),
                    idempotency_key.map(str::to_string),
                ));
                Ok("job-123".to_string())
            }
        }
        let rec = std::sync::Arc::new(MockJobs(std::sync::Mutex::new(None)));
        let mut engine = WorkflowEngine::new();
        engine.set_job_executor(rec.clone());
        engine.register(Workflow {
            name: "job_wf".to_string(),
            trigger: WorkflowTrigger::Manual,
            steps: vec![WorkflowStep {
                name: "enqueue".to_string(),
                action: StepAction::Job {
                    queue: "media-gen".into(),
                    kind: "video.generate".into(),
                    payload: json!({"prompt": "hi"}),
                    idempotency_key: Some("k1".into()),
                },
                condition: None,
                on_failure: FailurePolicy::Stop,
            }],
        });
        let exec = engine.execute("job_wf", HashMap::new()).await.unwrap();
        assert_eq!(
            exec.status,
            ExecutionStatus::Completed,
            "errors: {:?}",
            exec.errors
        );
        // The job id is exposed to later steps via context.
        assert_eq!(exec.context.get("job_id"), Some(&json!("job-123")));
        // The executor received the step's queue/kind/payload/key.
        let got = rec.0.lock().unwrap().clone().expect("enqueue must run");
        assert_eq!(got.0, "media-gen");
        assert_eq!(got.1, "video.generate");
        assert_eq!(got.2, json!({"prompt": "hi"}));
        assert_eq!(got.3.as_deref(), Some("k1"));
    }

    #[tokio::test]
    async fn job_step_without_executor_fails_loudly() {
        let engine = WorkflowEngine::new(); // no job executor injected
        engine.register(Workflow {
            name: "job_wf_noexec".to_string(),
            trigger: WorkflowTrigger::Manual,
            steps: vec![WorkflowStep {
                name: "enqueue".to_string(),
                action: StepAction::Job {
                    queue: "q".into(),
                    kind: "k".into(),
                    payload: json!({}),
                    idempotency_key: None,
                },
                condition: None,
                on_failure: FailurePolicy::Stop,
            }],
        });
        let exec = engine
            .execute("job_wf_noexec", HashMap::new())
            .await
            .unwrap();
        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert!(
            exec.errors
                .iter()
                .any(|e| e.contains("no executor configured")),
            "should fail loudly when no job executor is configured: {:?}",
            exec.errors
        );
    }
}
