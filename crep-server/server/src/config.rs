use std::fs;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ServerConfig {
    pub repo_path: String,
    pub branch_name: String,
    pub live_index_config: Option<LiveIndexConfig>,

    pub saved_index_path: String,
}

#[derive(Deserialize)]
pub enum LiveIndexConfig {
    WatchLiveUpdate(WatcherConfig),
    OnWebhookNotify,
}

#[derive(Deserialize)]
pub struct WatcherConfig {
    pub debounce_milliseconds: u64,
}

impl ServerConfig {
    pub fn new(config_path: &str) -> anyhow::Result<Self> {
        let yaml = fs::read_to_string(config_path)?;

        Ok(serde_yaml::from_str(&yaml)?)
    }
}
