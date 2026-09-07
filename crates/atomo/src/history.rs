//! Optional model history and audit lifecycle policies. Aggregate event sourcing is independent.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryMode {
    #[default]
    Full,
    Off,
    Retained,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditMode {
    #[default]
    Full,
    Metadata,
    Off,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryPolicy {
    pub mode: HistoryMode,
    pub max_age_secs: Option<u64>,
    pub max_bytes: Option<u64>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuditPolicy {
    pub mode: AuditMode,
    pub max_age_secs: Option<u64>,
    pub max_bytes: Option<u64>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    pub default: HistoryPolicy,
    pub models: HashMap<String, HistoryPolicy>,
    pub maintenance_interval_secs: u64,
    pub batch_size: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuditConfig {
    pub default: AuditPolicy,
    pub models: HashMap<String, AuditPolicy>,
    pub maintenance_interval_secs: u64,
    pub batch_size: u32,
}
impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            default: Default::default(),
            models: Default::default(),
            maintenance_interval_secs: 60,
            batch_size: 1000,
        }
    }
}
impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            default: Default::default(),
            models: Default::default(),
            maintenance_interval_secs: 60,
            batch_size: 1000,
        }
    }
}
fn validate_limits(age: Option<u64>, bytes: Option<u64>) -> Result<()> {
    if age == Some(0)
        || bytes == Some(0)
        || age.is_some_and(|v| v > i64::MAX as u64)
        || bytes.is_some_and(|v| v > i64::MAX as u64)
    {
        bail!("retention limits must be positive signed 64-bit values");
    }
    Ok(())
}
fn validate_worker(interval: u64, batch: u32) -> Result<()> {
    if interval == 0 || interval > 86400 || batch == 0 || batch > 10000 {
        bail!("maintenance interval must be 1..86400 seconds and batch size 1..10000");
    }
    Ok(())
}
impl HistoryConfig {
    pub fn validate_models(&self, schema: &crate::schema::Schema) -> Result<()> {
        self.validate()?;
        for model in self.models.keys() {
            if !schema.models.contains_key(model) {
                bail!("unknown history policy model: {model}");
            }
        }
        Ok(())
    }
    pub fn policy(&self, model: &str) -> &HistoryPolicy {
        self.models.get(model).unwrap_or(&self.default)
    }
    pub fn validate(&self) -> Result<()> {
        validate_worker(self.maintenance_interval_secs, self.batch_size)?;
        for policy in std::iter::once(&self.default).chain(self.models.values()) {
            validate_limits(policy.max_age_secs, policy.max_bytes)?;
            if policy.mode != HistoryMode::Retained
                && (policy.max_age_secs.is_some() || policy.max_bytes.is_some())
            {
                bail!("history retention limits require retained mode");
            }
            if policy.mode == HistoryMode::Retained
                && policy.max_age_secs.is_none()
                && policy.max_bytes.is_none()
            {
                bail!("retained history requires an age or byte limit");
            }
        }
        Ok(())
    }
    pub fn from_env() -> Result<Self> {
        let config: Self = match std::env::var("ATOMO_HISTORY_CONFIG") {
            Ok(raw) => serde_json::from_str(&raw)?,
            Err(std::env::VarError::NotPresent) => Self::default(),
            Err(e) => return Err(e.into()),
        };
        config.validate()?;
        Ok(config)
    }
}
impl AuditConfig {
    pub fn policy(&self, model: &str) -> &AuditPolicy {
        self.models.get(model).unwrap_or(&self.default)
    }
    pub fn validate(&self) -> Result<()> {
        validate_worker(self.maintenance_interval_secs, self.batch_size)?;
        for policy in std::iter::once(&self.default).chain(self.models.values()) {
            validate_limits(policy.max_age_secs, policy.max_bytes)?;
        }
        Ok(())
    }
    pub fn from_env() -> Result<Self> {
        let config: Self = match std::env::var("ATOMO_AUDIT_CONFIG") {
            Ok(raw) => serde_json::from_str(&raw)?,
            Err(std::env::VarError::NotPresent) => Self::default(),
            Err(e) => return Err(e.into()),
        };
        config.validate()?;
        Ok(config)
    }
}

/// Abort maintenance promptly when its owning server scope exits, including startup errors.
pub struct MaintenanceTask(pub tokio::task::JoinHandle<()>);
impl Drop for MaintenanceTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_preserve_history_and_audit() {
        assert_eq!(HistoryConfig::default().default.mode, HistoryMode::Full);
        assert_eq!(AuditConfig::default().default.mode, AuditMode::Full);
        let config: HistoryConfig =
            serde_json::from_str(r#"{"models":{"Counter":{"mode":"off"}}}"#).unwrap();
        assert_eq!(config.policy("Counter").mode, HistoryMode::Off);
        assert_eq!(config.policy("Ledger").mode, HistoryMode::Full);
    }
    #[test]
    fn reject_ambiguous_or_unbounded_retention() {
        assert!(validate_limits(Some(u64::MAX), None).is_err());
        assert!(validate_limits(None, Some(u64::MAX)).is_err());
        for raw in [
            r#"{"default":{"mode":"retained"}}"#,
            r#"{"default":{"max_bytes":10}}"#,
            r#"{"batch_size":0}"#,
            r#"{"default":{"mode":"retained","max_age_secs":0}}"#,
        ] {
            assert!(serde_json::from_str::<HistoryConfig>(raw)
                .unwrap()
                .validate()
                .is_err());
        }
        assert!(serde_json::from_str::<HistoryConfig>(r#"{"unexpected":1}"#).is_err());
    }
    #[tokio::test]
    async fn maintenance_owner_drop_cancels_task() {
        let task = tokio::spawn(std::future::pending::<()>());
        let abort = task.abort_handle();
        drop(MaintenanceTask(task));
        tokio::task::yield_now().await;
        assert!(abort.is_finished());
    }
}
