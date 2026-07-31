use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::time::sleep;

use crate::reindex_notify::reindex_signal::ReindexSignal;
use crate::reindex_notify::reindex_signal::ReindexSignalSender;
use crate::reindex_notify::repo_watcher::FsEvent;

// Whenever the directory is changed, debouncer gets notified.
// Debouncer will wake up the indexer N seconds later to batch multiple
// files that requires an indexing, or to handle when the same file
// gets modified multiple times in a short time frame.
pub struct Debouncer {
    file_paths_modified: Arc<Mutex<Vec<FsEvent>>>,
    is_timer_set: Arc<AtomicBool>,
    send_indexer_signal: ReindexSignalSender,
    handle: Handle,
    debounce_seconds: u64,
}

impl Debouncer {
    pub fn new(
        send_indexer_signal: ReindexSignalSender,
        file_paths_modified: Arc<Mutex<Vec<FsEvent>>>,
        debounce_seconds: u64,
    ) -> Self {
        Self {
            file_paths_modified,
            send_indexer_signal,
            is_timer_set: Arc::new(AtomicBool::new(false)),
            handle: Handle::current(),
            debounce_seconds,
        }
    }

    pub fn schedule_indexer_wakeup(&self, event: FsEvent) {
        self.file_paths_modified.lock().unwrap().push(event);

        // If the timer was not initiated yet, then we need to initiate it N seconds later.
        if !self
            .is_timer_set
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            let send_indexer_signal = self.send_indexer_signal.clone();
            let is_timer_set = self.is_timer_set.clone();
            let debounce_seconds = self.debounce_seconds;

            // Because the schedule_indexer_wakeup called from the notify callback, which is an OS
            // thread, tokio::spawn does not properly detect the tokio runtime. Because of this, we
            // have to manually pass the handle.
            self.handle.spawn(async move {
                sleep(Duration::from_secs(debounce_seconds)).await;

                send_indexer_signal
                    .send(ReindexSignal {
                        head_commit_id: todo!(),
                    })
                    .expect("Send wakeup signal");

                is_timer_set.store(false, std::sync::atomic::Ordering::Release);
            });
        }
    }
}
