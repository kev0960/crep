use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use crep_indexer::index::git_index::GitIndex;
use crep_indexer::index::not_committed_indexer::NotCommitedFilesIndexer;

use crate::config::ServerConfig;
use crate::search::repo_pool::RepoPool;
use crate::search::search_cache::SearchCache;

#[derive(Clone)]
pub struct ServerContext {
    pub index: Arc<GitIndex>,
    pub not_commited_file_indexer: Arc<NotCommitedFilesIndexer>,
    pub repo_pool: Arc<RepoPool>,
    pub search_cache: Arc<SearchCache>,
}

impl ServerContext {
    pub fn new(
        config: &ServerConfig,
        not_commited_file_indexer: Arc<NotCommitedFilesIndexer>,
    ) -> anyhow::Result<Self> {
        let index = GitIndex::load(&PathBuf::from(&config.saved_index_path))?;

        Ok(Self {
            index: Arc::new(index),
            not_commited_file_indexer,
            repo_pool: Arc::new(RepoPool::new(&config.repo_path)),
            search_cache: Arc::new(SearchCache::new(
                NonZeroUsize::new(1024).unwrap(),
            )),
        })
    }
}
