use std::path::Path;
use std::sync::Arc;

use crep_indexer::index::git_indexer::GitIndexer;
use tokio::sync::RwLock;

use crate::indexer::index::SearchIndex;
use crate::indexer::index::SearchIndexGuard;
use crate::reindex_notify::reindex_signal::ReindexSignal;
use crate::reindex_notify::reindex_signal::ReindexSignalReceiver;
use crate::reindex_notify::reindex_signal::ReindexSignalSender;
use crate::search::search_cache::SearchCache;

pub struct Indexer {
    index: Arc<RwLock<SearchIndex>>,
    send_reindex_signal: ReindexSignalSender,
}

impl Indexer {
    pub fn new(
        indexer: GitIndexer,
        repo_path: &Path,
        send_reindex_signal: ReindexSignalSender,
    ) -> Self {
        Self {
            index: Arc::new(RwLock::new(SearchIndex::new(indexer, repo_path))),
            send_reindex_signal,
        }
    }

    pub fn request_reindex(&self, reindex_singal: ReindexSignal) {
        self.send_reindex_signal.send(reindex_singal).unwrap();
    }

    pub async fn get_search_index(&self) -> SearchIndexGuard {
        SearchIndexGuard(Arc::new(self.index.clone().read_owned().await))
    }

    pub fn spawn_re_indexer(
        &self,
        recv_indexer_signal: ReindexSignalReceiver,
        search_cache: Arc<SearchCache>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(
            ReIndexer {
                recv_indexer_signal,
                index: self.index.clone(),
                search_cache,
            }
            .handle_re_index(),
        )
    }
}

struct ReIndexer {
    recv_indexer_signal: ReindexSignalReceiver,
    index: Arc<RwLock<SearchIndex>>,
    search_cache: Arc<SearchCache>,
}

impl ReIndexer {
    async fn handle_re_index(mut self) {
        while let Some(signal) = self.recv_indexer_signal.recv().await {
            let index = self.index.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut index = index.blocking_write();
                index.do_incremental_index(&signal.head_commit_id)
            })
            .await;

            match result {
                Ok(Ok(true)) => {
                    self.search_cache.evict_cache_after_reindex();
                }
                Ok(Err(e)) => {
                    eprintln!("Failed reindex {:?}", e);
                }
                Err(e) => {
                    eprintln!("Reindex task failed {:?}", e);
                }
                _ => {}
            }
        }
    }
}
