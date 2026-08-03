use std::path::PathBuf;
use std::sync::Arc;

use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::Watcher;
use notify::event::ModifyKind;

use crate::config::ServerConfig;
use crate::reindex_notify::debouncer::Debouncer;
use crate::reindex_notify::reindex_signal::ReindexSignalSender;

pub struct RepoWatcher {
    pub watcher: RecommendedWatcher,
}

impl RepoWatcher {
    pub fn new(
        config: &ServerConfig,
        send_indexer_signal: ReindexSignalSender,
    ) -> anyhow::Result<Self> {
        let debouncer = Arc::new(Debouncer::new(config, send_indexer_signal));
        let watch_path = format!("{}/.git", config.repo_path);
        let watch_path_buf = PathBuf::from(&watch_path);

        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    if !is_modify_event(&event) {
                        return;
                    }

                    if !event.paths.iter().any(|modified_path| {
                        let rel_path_in_repo = modified_path
                            .to_str()
                            .unwrap()
                            .strip_prefix(&watch_path)
                            .unwrap();

                        WATCH_GIT_PATHS
                            .iter()
                            .any(|p| rel_path_in_repo.starts_with(p))
                    }) {
                        return;
                    }

                    debouncer.schedule_git_change_check();
                }
            })?;

        watcher.watch(&watch_path_buf, notify::RecursiveMode::Recursive)?;

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

const WATCH_GIT_PATHS: &[&str] = &[
    "/HEAD",
    "/refs/heads",
    "/packed-refs",
    "/logs/HEAD",
    "/logs/refs/heads",
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tokio::sync::mpsc::unbounded_channel;

    use crate::{
        config::{LiveIndexConfig, WatcherConfig},
        reindex_notify::reindex_signal::ReindexSignal,
    };

    use super::*;
    use std::time::Duration;

    fn run(
        cwd: &Path,
        args: &[&str],
    ) -> (i32, /*stdout=*/ String, /*stderr=*/ String) {
        let out = std::process::Command::new(args[0])
            .args(&args[1..])
            .current_dir(cwd)
            .output()
            .expect("spawn ok");

        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    #[tokio::test]
    async fn test_watch_git_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo_path = dir.path();

        let _ = run(repo_path, &["git", "init"]);
        let _ = run(
            repo_path,
            &["git", "config", "--local", "user.email", "jaebum@test.com"],
        );
        let _ = run(
            repo_path,
            &["git", "config", "--local", "user.name", "Jaebum"],
        );

        std::fs::write(repo_path.join("file.txt"), "a\nbc\ndef\ndefa").unwrap();
        run(repo_path, &["git", "add", "."]);
        run(repo_path, &["git", "commit", "-m", "init"]);

        let (send_indexer_signal, recv_indexer_signal) =
            unbounded_channel::<ReindexSignal>();

        // Now let's add a watcher.
        let _repo_watcher = RepoWatcher::new(
            &ServerConfig {
                repo_path: repo_path.to_str().unwrap().to_string(),
                branch_name: "main".to_owned(),
                live_index_config: Some(LiveIndexConfig::WatchLiveUpdate(
                    WatcherConfig {
                        debounce_milliseconds: 300,
                    },
                )),
                saved_index_path: "".to_owned(),
            },
            send_indexer_signal,
        );

        std::fs::write(repo_path.join("file2.txt"), "a\nbc\ndef\ndefa")
            .unwrap();

        tokio::time::sleep(Duration::from_millis(600)).await;

        // Writing the file shouldn't trigger re-index as the git is not modified.
        assert!(recv_indexer_signal.is_empty());

        run(repo_path, &["git", "add", "."]);
        run(repo_path, &["git", "commit", "-m", "second"]);

        tokio::time::sleep(Duration::from_millis(600)).await;

        assert!(!recv_indexer_signal.is_empty());
    }
}
