use std::sync::Arc;

use crep_indexer::index::git_indexer::GitIndexer;
use tokio::sync::RwLock;

use crate::indexer::index::SearchIndex;
use crate::indexer::index::SearchIndexGuard;
use crate::reindex_notify::reindex_signal::ReindexSignal;
use crate::reindex_notify::reindex_signal::ReindexSignalReceiver;
use crate::reindex_notify::reindex_signal::ReindexSignalSender;

pub struct Indexer {
    index: Arc<RwLock<SearchIndex>>,
    send_reindex_signal: ReindexSignalSender,
}

impl Indexer {
    pub fn new(
        indexer: GitIndexer,
        send_reindex_signal: ReindexSignalSender,
    ) -> Self {
        Self {
            index: Arc::new(RwLock::new(SearchIndex::new(indexer))),
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
    recv_indexer_signal: ReindexSignalReceiver,
    index: Arc<RwLock<SearchIndex>>,
}

impl ReIndexer {
    async fn handle_re_index(mut self) {
        while let Some(_signal) = self.recv_indexer_signal.recv().await {
            let mut index = self.index.write().await;

            index.refresh_all_words()
        }
    }
}
