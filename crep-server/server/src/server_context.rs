use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crep_indexer::index::git_index::GitIndex;
use git2::Repository;

use crate::config::ServerConfig;
use crate::search::search_cache::SearchCache;

#[derive(Clone)]
pub struct ServerContext {
    pub index: Arc<GitIndex>,
    pub repo: Arc<Mutex<Repository>>,
    pub search_cache: Arc<SearchCache>,
}

impl ServerContext {
    pub fn new(config: &ServerConfig) -> anyhow::Result<Self> {
        let index = GitIndex::load(&PathBuf::from(&config.saved_index_path))?;
        let repo = Repository::open(&config.repo_path).unwrap();

        Ok(Self {
            index: Arc::new(index),
            repo: Arc::new(Mutex::new(repo)),
            search_cache: Arc::new(SearchCache::new(
                NonZeroUsize::new(1024).unwrap(),
            )),
        })
    }
}
