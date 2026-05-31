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
    Mutation { query: String, variables: HashMap<String, Value> },
    /// Call a WASM plugin function
    Plugin { plugin_name: String, function: String },
    /// Send an HTTP request
    Http { method: String, url: String, body: Option<Value> },
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
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self { workflows: std::sync::RwLock::new(HashMap::new()) }
    }

    pub fn register(&self, workflow: Workflow) {
        self.workflows.write().unwrap().insert(workflow.name.clone(), workflow);
    }

    /// List registered workflow names
    pub fn list(&self) -> Vec<String> {
        self.workflows.read().unwrap().keys().cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<Workflow> {
        self.workflows.read().unwrap().get(name).cloned()
    }

    /// Execute a workflow with initial context
    pub async fn execute(&self, name: &str, initial_context: HashMap<String, Value>) -> Result<WorkflowExecution> {
        let workflow = self.workflows.read().unwrap().get(name).cloned()
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
            match execute_step(&step.action, &mut execution.context).await {
                Ok(()) => {}
                Err(e) => {
                    execution.errors.push(format!("Step '{}': {}", step.name, e));
                    match &step.on_failure {
                        FailurePolicy::Stop => {
                            execution.status = ExecutionStatus::Failed;
                            return Ok(execution);
                        }
                        FailurePolicy::Continue => continue,
                        FailurePolicy::Retry { max_attempts } => {
                            let mut success = false;
                            for _ in 0..*max_attempts {
                                if execute_step(&step.action, &mut execution.context).await.is_ok() {
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
    pub fn start_event_listener(self: Arc<Self>, mut rx: tokio::sync::broadcast::Receiver<crate::events::ModelEvent>) {
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
                let scheduled: Vec<(String, String)> = self.workflows.read().unwrap().values()
                    .filter_map(|w| match &w.trigger {
                        WorkflowTrigger::Schedule { cron } => Some((w.name.clone(), cron.clone())),
                        _ => None,
                    })
                    .collect();
                for (name, cron_expr) in scheduled {
                    match cron::Schedule::from_str(&cron_expr) {
                        Ok(schedule) => {
                            if let Some(next) = schedule.after(&last_tick).next() {
                                if next <= now {
                                    if let Err(e) = self.execute(&name, HashMap::new()).await {
                                        tracing::error!(workflow = %name, error = %e, "Scheduled workflow failed");
                                    }
                                }
                            }
                        }
                        Err(e) => tracing::warn!(workflow = %name, error = %e, "Invalid cron expression"),
                    }
                }
                last_tick = now;
            }
        });
    }
}

fn evaluate_condition(cond: &Condition, context: &HashMap<String, Value>) -> bool {
    let val = context.get(&cond.field).unwrap_or(&Value::Null);
    match cond.operator.as_str() {
        "eq" => val == &cond.value,
        "neq" => val != &cond.value,
        "gt" => val.as_f64().unwrap_or(0.0) > cond.value.as_f64().unwrap_or(0.0),
        "lt" => val.as_f64().unwrap_or(0.0) < cond.value.as_f64().unwrap_or(0.0),
        "contains" => val.as_str().unwrap_or("").contains(cond.value.as_str().unwrap_or("")),
        _ => true,
    }
}

async fn execute_step(action: &StepAction, context: &mut HashMap<String, Value>) -> Result<()> {
    match action {
        StepAction::Delay { seconds } => {
            tokio::time::sleep(std::time::Duration::from_secs(*seconds)).await;
        }
        StepAction::SetVariable { key, value } => {
            context.insert(key.clone(), value.clone());
        }
        StepAction::Http { method, url, body: _ } => {
            tracing::info!(method = %method, url = %url, "Workflow HTTP step");
        }
        StepAction::Mutation { query, variables: _ } => {
            tracing::info!(query = %query, "Workflow mutation step");
        }
        StepAction::Plugin { plugin_name, function } => {
            tracing::info!(plugin = %plugin_name, function = %function, "Workflow plugin step");
        }
    }
    Ok(())
}
