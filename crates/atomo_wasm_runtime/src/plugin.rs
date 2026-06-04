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
    /// HTTP routes this plugin serves. atomo-server mounts each under
    /// `/ext/<plugin-name><path>` and dispatches matching requests to the plugin.
    #[serde(default)]
    pub routes: Vec<RouteDef>,
}

/// A plugin-declared HTTP route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDef {
    /// HTTP method, e.g. "GET" / "POST" (case-insensitive).
    pub method: String,
    /// Path under the plugin's mount, e.g. `/debit` -> `/ext/<plugin>/debit`.
    pub path: String,
    /// When true, the request must carry a valid JWT; the verified principal is
    /// passed to the handler. Default false (public).
    #[serde(default)]
    pub auth: bool,
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
    pub fn ensure(
        perms: &std::collections::HashSet<Permission>,
        required: &Permission,
    ) -> anyhow::Result<()> {
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
