use std::fs;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ServerConfig {
    pub repo_path: String,
    pub saved_index_path: String,
}

impl ServerConfig {
    pub fn new(config_path: &str) -> anyhow::Result<Self> {
        let yaml = fs::read_to_string(config_path)?;

        Ok(serde_yaml::from_str(&yaml)?)
    }
}
