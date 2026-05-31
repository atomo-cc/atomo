use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use atomo_wasm_runtime::{PluginManifest, WasmPlugin, WasmRuntime};
use tracing::info;

pub struct WasmPluginManager {
    runtime: WasmRuntime,
    plugins: HashMap<String, WasmPlugin>,
    plugin_dir: PathBuf,
}

impl WasmPluginManager {
    pub fn new(plugin_dir: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            runtime: WasmRuntime::new()?,
            plugins: HashMap::new(),
            plugin_dir: plugin_dir.into(),
        })
    }

    /// Discover and load all plugins from the plugin directory
    pub async fn discover_and_load(&mut self) -> Result<Vec<String>> {
        let mut loaded = Vec::new();
        let dir = &self.plugin_dir;
        if !dir.exists() {
            return Ok(loaded);
        }

        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(name) = self.load_plugin_from_dir(&path).await {
                    loaded.push(name);
                }
            }
        }
        Ok(loaded)
    }

    async fn load_plugin_from_dir(&mut self, dir: &Path) -> Result<String> {
        let manifest_path = dir.join("plugin.toml");
        let manifest_content = tokio::fs::read_to_string(&manifest_path).await?;
        let manifest: PluginManifest = toml::from_str(&manifest_content)?;
        let wasm_path = dir.join(&manifest.entry_point);
        let name = manifest.name.clone();
        info!(plugin = %name, "Loading WASM plugin");
        let plugin = self.runtime.load_plugin(&wasm_path, &manifest).await?;
        self.plugins.insert(name.clone(), plugin);
        Ok(name)
    }

    /// Execute a function on a named plugin
    pub fn call(
        &mut self,
        plugin_name: &str,
        function: &str,
        args: &[wasmtime::Val],
    ) -> Result<Vec<wasmtime::Val>> {
        let plugin = self
            .plugins
            .get_mut(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;
        plugin.call_function(function, args)
    }

    /// Call a hook with JSON marshalling
    pub fn call_hook(&mut self, plugin_name: &str, hook: &str, input_json: &str) -> Result<Option<String>> {
        let plugin = self.plugins.get_mut(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;
        plugin.call_hook(hook, input_json)
    }

    pub fn loaded_plugins(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }
}
