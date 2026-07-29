use crate::config::LiveIndexConfig;
use crate::config::ServerConfig;
use crate::indexer::indexer::Indexer;
use crate::watch::debouncer::Debouncer;
use crate::watch::repo_watcher::RepoWatcher;

use std::sync::Arc;
use std::sync::Mutex;

use crep_indexer::index::git_indexer::GitIndexer;
use tokio::sync::mpsc::unbounded_channel;

pub fn init_watcher_and_indexer(
    server_config: &ServerConfig,
    git_indexer: GitIndexer,
) -> (Indexer, Option<RepoWatcher>) {
    let (send_indexer_signal, recv_indexer_signal) = unbounded_channel::<()>();
    let file_paths_modified = Arc::new(Mutex::new(Vec::new()));

    let repo_watcher = match &server_config.live_index_config {
        Some(LiveIndexConfig::WatchLiveUpdate(watch_config)) => {
            Some(RepoWatcher {
                debouncer: Arc::new(Debouncer::new(
                    send_indexer_signal,
                    file_paths_modified.clone(),
                    watch_config.debounce_seconds,
                )),
                watcher: None,
            })
        }
        _ => None,
    };

    let indexer = Indexer::new(git_indexer);
    indexer.spawn_re_indexer(recv_indexer_signal);

    (indexer, repo_watcher)
}
