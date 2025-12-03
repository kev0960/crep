use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use ahash::AHashSet;
use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::Watcher;
use notify::event::ModifyKind;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::sleep;

use crate::watch::ignore_checker::IgnoreChecker;

pub struct WatcherConfig {
    pub debounce_seconds: u64,
}

pub fn init_watcher_and_indexer(
    watcher_config: WatcherConfig,
) -> (RepoWatcher, Indexer) {
    let (send_indexer_signal, recv_indexer_signal) = unbounded_channel::<()>();
    let file_paths_modified = Arc::new(Mutex::new(Vec::new()));

    let debouncer = Arc::new(Debouncer::new(
        send_indexer_signal,
        file_paths_modified.clone(),
        watcher_config.debounce_seconds,
    ));

    let indexer = Indexer::new(recv_indexer_signal, file_paths_modified);
    (
        RepoWatcher {
            debouncer,
            watcher: None,
        },
        indexer,
    )
}

pub struct RepoWatcher {
    debouncer: Arc<Debouncer>,
    watcher: Option<RecommendedWatcher>,
}

#[derive(Debug)]
enum FsEventType {
    Create,
    Remove,
    Modify,
}

#[derive(Debug)]
struct FsEvent(FsEventType, Vec<PathBuf>);

impl FsEvent {
    fn from(kind: EventKind, paths: Vec<PathBuf>) -> Self {
        match kind {
            EventKind::Create(_) => FsEvent(FsEventType::Create, paths),
            EventKind::Remove(_) => FsEvent(FsEventType::Remove, paths),
            EventKind::Modify(ModifyKind::Data(_)) => {
                FsEvent(FsEventType::Modify, paths)
            }
            EventKind::Modify(ModifyKind::Name(_)) => {
                FsEvent(FsEventType::Create, paths)
            }
            _ => panic!("Unsupported event kind!"),
        }
    }
}

impl RepoWatcher {
    pub fn start_watch(
        &mut self,
        path: &Path,
        ignore_checker: IgnoreChecker,
    ) -> anyhow::Result<()> {
        let debouncer = self.debouncer.clone();
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    if !is_modify_event(&event) {
                        return;
                    }

                    let paths: Vec<PathBuf> = event
                        .paths
                        .into_iter()
                        .filter(|path| !ignore_checker.is_ignored(path))
                        .collect();

                    // No need to wake up if all paths are ignored.
                    if paths.is_empty() {
                        return;
                    }

                    let event_kind = event.kind;
                    debouncer.schedule_indexer_wakeup(FsEvent::from(
                        event_kind, paths,
                    ));
                }
            })?;

        watcher.watch(path, notify::RecursiveMode::Recursive)?;

        self.watcher = Some(watcher);

        Ok(())
    }
}

// Whenever the directory is changed, debouncer gets notified.
// Debouncer will wake up the indexer N seconds later to batch multiple
// files that requires an indexing, or to handle when the same file
// gets modified multiple times in a short time frame.
struct Debouncer {
    file_paths_modified: Arc<Mutex<Vec<FsEvent>>>,
    is_timer_set: Arc<AtomicBool>,
    send_indexer_signal: UnboundedSender<()>,
    handle: Handle,
    debounce_seconds: u64,
}

impl Debouncer {
    fn new(
        send_indexer_signal: UnboundedSender<()>,
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

    fn schedule_indexer_wakeup(&self, event: FsEvent) {
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

                send_indexer_signal.send(()).expect("Send wakeup signal");
                is_timer_set.store(false, std::sync::atomic::Ordering::Release);
            });
        }
    }
}

pub struct Indexer {
    recv_indexer_signal: UnboundedReceiver<()>,
    file_events: Arc<Mutex<Vec<FsEvent>>>,
}

impl Indexer {
    fn new(
        recv_indexer_signal: UnboundedReceiver<()>,
        file_events: Arc<Mutex<Vec<FsEvent>>>,
    ) -> Self {
        Self {
            recv_indexer_signal,
            file_events,
        }
    }

    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.handle_index())
    }

    async fn handle_index(mut self) {
        while let Some(()) = self.recv_indexer_signal.recv().await {
            let file_events = {
                self.file_events
                    .lock()
                    .unwrap()
                    .drain(..)
                    .collect::<Vec<_>>()

                // Drop the mutex.
            };

            // Merge all paths to check. If the file was created and then deleted later, then no
            // need to visit the file again.
            let mut modified_files = AHashSet::new();
            let mut created_files = AHashSet::new();

            for mut event in file_events {
                match event.0 {
                    FsEventType::Create => {
                        created_files.extend(event.1.drain(..));
                    }
                    FsEventType::Modify => {
                        modified_files.extend(event.1.drain(..));
                    }
                    FsEventType::Remove => {
                        for removed_file in event.1 {
                            if created_files.contains(&removed_file) {
                                created_files.remove(&removed_file);
                                modified_files.remove(&removed_file);
                            } else {
                                modified_files.insert(removed_file);
                            }
                        }
                    }
                }
            }

            modified_files.extend(created_files.drain());

            tokio::task::spawn_blocking(move || {
                println!("Modified paths: {:?}", modified_files);
            });
        }
    }
}

fn is_modify_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Name(_))
    )
}
