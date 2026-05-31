use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub entry_point: String,
    pub permissions: Vec<Permission>,
    /// Plugin runtime: compiled wasm (default) or a Javy-built JS module.
    #[serde(default)]
    pub runtime: PluginRuntime,
}

/// Which runtime executes the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntime {
    /// Compiled WebAssembly with exported hook functions (Tier 2).
    #[default]
    Wasm,
    /// JavaScript compiled with Javy; runs over WASI stdin/stdout (Tier 1).
    Js,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    ReadEvents,
    WriteEvents,
    ReadDatabase,
    WriteDatabase,
    HttpRequests,
}

impl Permission {
    /// The single permission-check seam used by BOTH plugin tiers: compiled host functions
    /// (`runtime.rs`) and JS effects (`atomo_server::wasm_plugins`). Centralizing it keeps the
    /// gating logic and the error wording consistent — a plugin that lacks `required` is denied
    /// the same way regardless of tier.
    pub fn ensure(perms: &std::collections::HashSet<Permission>, required: &Permission) -> anyhow::Result<()> {
        if perms.contains(required) {
            Ok(())
        } else {
            anyhow::bail!("Permission denied: {:?} required", required)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginContext {
    pub event_data: serde_json::Value,
    pub metadata: std::collections::HashMap<String, String>,
}
