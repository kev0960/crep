use std::sync::Arc;

use crep_indexer::index::git_indexer::GitIndexer;
use tokio::sync::RwLock;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::indexer::index::SearchIndex;
use crate::indexer::index::SearchIndexGuard;

pub struct Indexer {
    index: Arc<RwLock<SearchIndex>>,
}

impl Indexer {
    pub fn new(indexer: GitIndexer) -> Self {
        Self {
            index: Arc::new(RwLock::new(SearchIndex::new(indexer))),
        }
    }

    pub async fn get_search_index(&self) -> SearchIndexGuard {
        SearchIndexGuard(Arc::new(self.index.clone().read_owned().await))
    }

    pub fn spawn_re_indexer(
        &self,
        recv_indexer_signal: UnboundedReceiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(
            ReIndexer {
                recv_indexer_signal,
                index: self.index.clone(),
            }
            .handle_re_index(),
        )
    }
}

struct ReIndexer {
    recv_indexer_signal: UnboundedReceiver<()>,
    index: Arc<RwLock<SearchIndex>>,
}

impl ReIndexer {
    async fn handle_re_index(mut self) {
        while let Some(()) = self.recv_indexer_signal.recv().await {
            let mut index = self.index.write().await;

            index.refresh_all_words()
        }
    }
}
