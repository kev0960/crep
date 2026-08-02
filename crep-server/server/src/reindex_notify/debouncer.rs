use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::time::sleep;

use crate::config::LiveIndexConfig;
use crate::config::ServerConfig;
use crate::reindex_notify::reindex_signal::ReindexSignal;
use crate::reindex_notify::reindex_signal::ReindexSignalSender;

// Whenever the .git directory is changed, debouncer gets notified.
// Debouncer will wake up the N seconds later to check git Repository
// to send the current
pub struct Debouncer {
    is_timer_set: Arc<AtomicBool>,
    send_indexer_signal: ReindexSignalSender,
    handle: Handle,
    debounce_milli_seconds: u64,
    watched_repo: Arc<Mutex<git2::Repository>>,
    watched_branch: String,
}

impl Debouncer {
    pub fn new(
        config: &ServerConfig,
        send_indexer_signal: ReindexSignalSender,
    ) -> Self {
        let debounce_milliseconds = match &config.live_index_config {
            Some(LiveIndexConfig::WatchLiveUpdate(config)) => {
                config.debounce_milliseconds
            }
            _ => panic!(""),
        };

        let watched_repo = Arc::new(Mutex::new(
            git2::Repository::open(&config.repo_path).unwrap(),
        ));

        Self {
            send_indexer_signal,
            is_timer_set: Arc::new(AtomicBool::new(false)),
            handle: Handle::current(),
            debounce_milli_seconds: debounce_milliseconds,
            watched_branch: config.branch_name.clone(),
            watched_repo,
        }
    }

    pub fn schedule_git_change_check(&self) {
        // If the timer was not initiated yet, then we need to initiate it N seconds later.
        if !self
            .is_timer_set
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            let send_indexer_signal = self.send_indexer_signal.clone();
            let is_timer_set = self.is_timer_set.clone();
            let debounce_milliseconds = self.debounce_milli_seconds;

            let repo = self.watched_repo.clone();
            let watched_branch = self.watched_branch.clone();

            // Because the schedule_indexer_wakeup called from the notify callback, which is an OS
            // thread, tokio::spawn does not properly detect the tokio runtime. Because of this, we
            // have to manually pass the handle.
            self.handle.spawn(async move {
                sleep(Duration::from_millis(debounce_milliseconds)).await;

                let head_commit_id = repo
                    .lock()
                    .unwrap()
                    .find_reference(&format!("refs/heads/{}", &watched_branch))
                    .unwrap()
                    .peel_to_commit()
                    .map(|commit| commit.id())
                    .unwrap()
                    .to_string();

                send_indexer_signal
                    .send(ReindexSignal { head_commit_id })
                    .expect("Send wakeup signal");

                is_timer_set.store(false, std::sync::atomic::Ordering::Release);
            });
        }
    }
}
