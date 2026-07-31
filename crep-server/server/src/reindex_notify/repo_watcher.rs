use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::Watcher;
use notify::event::ModifyKind;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::WatcherConfig;
use crate::reindex_notify::debouncer::Debouncer;
use crate::reindex_notify::reindex_signal::ReindexSignalSender;

#[derive(Debug)]
pub enum FsEventType {
    Create,
    Remove,
    Modify,
}

#[derive(Debug)]
pub struct FsEvent(pub FsEventType, pub Vec<PathBuf>);

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

pub struct RepoWatcher {
    pub watcher: RecommendedWatcher,
}

impl RepoWatcher {
    pub fn new(
        config: &WatcherConfig,
        watch_path: PathBuf,
        send_indexer_signal: ReindexSignalSender,
    ) -> anyhow::Result<Self> {
        let debouncer = Arc::new(Debouncer::new(
            send_indexer_signal,
            Arc::new(Mutex::new(Vec::new())),
            config.debounce_seconds,
        ));

        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    if !is_modify_event(&event) {
                        return;
                    }

                    let paths: Vec<PathBuf> = event.paths.into_iter().collect();

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

        watcher.watch(&watch_path, notify::RecursiveMode::Recursive)?;

        Ok(Self { watcher })
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
