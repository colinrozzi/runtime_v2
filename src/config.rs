use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorConfig {
    pub name: String,
    pub component_path: PathBuf,
    pub interfaces: Vec<String>,
    pub http_port: Option<u16>,
}

impl ActorConfig {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ActorConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn supports_http(&self) -> bool {
        self.interfaces.contains(&"ntwk:simple-http-actor/http-actor".to_string())
    }
}