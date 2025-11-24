use std::path::Path;

use ignore::{WalkBuilder, gitignore::Gitignore};
use notify::RecommendedWatcher;

struct RepoWatcher {
    ignore: Gitignore,
    watcher: RecommendedWatcher,
}

impl RepoWatcher {
    fn new(path: &str) -> Self {}
}
