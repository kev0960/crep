use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crep_indexer::index::git_index::GitIndex;
use git2::Repository;

use crate::config::ServerConfig;

#[derive(Clone)]
pub struct ServerContext {
    pub index: Arc<GitIndex>,
    pub repo: Arc<Mutex<Repository>>,
}

impl ServerContext {
    pub fn new(config: &ServerConfig) -> anyhow::Result<Self> {
        let index = GitIndex::load(&PathBuf::from(&config.saved_index_path))?;
        let repo = Repository::open(&config.repo_path).unwrap();

        Ok(Self {
            index: Arc::new(index),
            repo: Arc::new(Mutex::new(repo)),
        })
    }
}
