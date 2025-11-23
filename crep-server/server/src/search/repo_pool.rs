use std::sync::{Arc, Mutex};

use git2::Repository;

#[derive(Clone)]
pub struct RepoPool {
    pub repos: Vec<Arc<Mutex<Repository>>>,
}

impl RepoPool {
    pub fn new(path: &str) -> Self {
        let num_threads = rayon::current_num_threads();
        assert!(num_threads > 0);

        RepoPool {
            repos: (0..num_threads)
                .map(|_| Arc::new(Mutex::new(Repository::open(path).unwrap())))
                .collect::<Vec<_>>(),
        }
    }
}
