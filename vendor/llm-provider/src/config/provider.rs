use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            api_key: None,
            base_url: None,
            timeout_ms: 60_000,
            max_retries: 3,
            extra: HashMap::new(),
        }
    }
}
