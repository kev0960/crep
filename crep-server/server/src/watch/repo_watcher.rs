use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::Watcher;
use notify::event::ModifyKind;

use crate::watch::debouncer::Debouncer;
use crate::watch::ignore_checker::IgnoreChecker;

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
    pub debouncer: Arc<Debouncer>,
    pub watcher: Option<RecommendedWatcher>,
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

fn is_modify_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Name(_))
    )
}
