use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::config::ServerConfig;
use crate::indexer::indexer::Indexer;
use crate::search::repo_pool::RepoPool;
use crate::search::search_cache::SearchCache;

#[derive(Clone)]
pub struct ServerContext {
    pub indexer: Arc<Indexer>,
    pub repo_pool: Arc<RepoPool>,
    pub search_cache: Arc<SearchCache>,
}

impl ServerContext {
    pub fn new(
        config: &ServerConfig,
        indexer: Arc<Indexer>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            indexer,
            repo_pool: Arc::new(RepoPool::new(&config.repo_path)),
            search_cache: Arc::new(SearchCache::new(
                NonZeroUsize::new(1024).unwrap(),
            )),
        })
    }
}
