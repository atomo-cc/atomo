use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub entry_point: String,
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    ReadEvents,
    WriteEvents,
    ReadDatabase,
    WriteDatabase,
    HttpRequests,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginContext {
    pub event_data: serde_json::Value,
    pub metadata: std::collections::HashMap<String, String>,
}
