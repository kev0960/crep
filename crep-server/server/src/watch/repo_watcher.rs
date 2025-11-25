use std::path::Path;

use ignore::{WalkBuilder, gitignore::Gitignore};
use notify::RecommendedWatcher;

struct RepoWatcher {}

impl RepoWatcher {
    fn new(path: &str) -> Self {
        Self {}
    }
}
