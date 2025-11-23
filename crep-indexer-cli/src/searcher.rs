use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use chrono::DateTime;
use chrono::Utc;
use crep_indexer::index::git_index::GitIndex;
use crep_indexer::index::git_indexer::CommitIndex;
use crep_indexer::index::git_indexer::FileId;
use crep_indexer::search::git_searcher::GitSearcher;
use crep_indexer::search::git_searcher::Query;
use crep_indexer::search::git_searcher::SearchOption;
use crep_indexer::search::result::search_result::RepoReader;
use crep_indexer::search::result::search_result::SearchResult;
use git2::Oid;
use git2::Repository;
use log::debug;
use log::info;
use rayon::prelude::*;

pub struct Searcher<'a> {
    pool: RepoPool,
    index: &'a GitIndex,
    searcher: GitSearcher<'a>,
}

struct RepoPool {
    repos: Vec<Arc<Mutex<Repository>>>,
}

impl RepoPool {
    fn new(num_threads: usize, path: &str) -> Self {
        let mut repos = vec![];
        for _ in 0..num_threads {
            repos.push(Arc::new(Mutex::new(
                Repository::open(Path::new(path)).unwrap(),
            )));
        }

        Self { repos }
    }
}

impl<'a> Searcher<'a> {
    pub fn new(index: &'a GitIndex, path: &str) -> Self {
        assert!(rayon::current_num_threads() > 0);

        Self {
            pool: RepoPool::new(rayon::current_num_threads(), path),
            index,
            searcher: GitSearcher::new(index),
        }
    }

    pub fn handle_query(
        &mut self,
        query: &Query,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let raw_result_start = Instant::now();

        let raw_results = self.searcher.search(
            query,
            Some(SearchOption {
                max_num_to_find: None,
            }),
        );

        info!(
            "Raw result end: {}",
            Instant::now().duration_since(raw_result_start).as_millis()
        );

        if raw_results.is_err() {
            anyhow::bail!("{raw_results:?}")
        }

        let raw_results = raw_results.unwrap();

        let to_search_result_start = Instant::now();
        let results = raw_results
            .par_iter()
            .map_init(
                || ThreadSafeRepoReader {
                    repo: self
                        .pool
                        .repos
                        .get(rayon::current_thread_index().unwrap())
                        .unwrap()
                        .clone(),
                    file_id_to_path: &self.index.file_id_to_path,
                    commit_index_to_commit_id: &self
                        .index
                        .commit_index_to_commit_id,
                },
                |reader, result| {
                    debug!(
                        "Checking {result:?} at {}",
                        self.index.file_id_to_path[result.file_id as usize]
                    );

                    SearchResult::new(reader, result).unwrap()
                },
            )
            .filter(|res| res.is_some())
            .take_any(100)
            .collect::<Vec<_>>();

        info!(
            "Search result end: {}",
            Instant::now()
                .duration_since(to_search_result_start)
                .as_millis()
        );

        Ok(results.into_iter().map(|s| s.unwrap()).collect())
    }

    pub fn get_commit_info(
        &self,
        commit_index: usize,
    ) -> anyhow::Result<CommitInfo> {
        let commit_id = Oid::from_bytes(
            &self.index.commit_index_to_commit_id[commit_index],
        )?;

        let repo = self.pool.repos.first().unwrap().lock().unwrap();
        let commit = repo.find_commit(commit_id)?;

        Ok(CommitInfo {
            commit_id: commit_id.to_string(),
            commit_time: DateTime::from_timestamp_secs(commit.time().seconds())
                .ok_or_else(|| anyhow::anyhow!("invalid commit timestamp"))?,
            is_commit_head: self.index.commit_index_to_commit_id.len() - 1
                == commit_index,
        })
    }
}

pub struct CommitInfo {
    pub commit_id: String,
    pub commit_time: DateTime<Utc>,
    pub is_commit_head: bool,
}

impl CommitInfo {
    pub fn display_simple<'a>(&'a self) -> SimpleCommitInfo<'a> {
        SimpleCommitInfo(self)
    }
}

pub struct SimpleCommitInfo<'a>(&'a CommitInfo);
impl<'a> fmt::Display for SimpleCommitInfo<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.0;

        if c.is_commit_head {
            write!(f, "HEAD (at {})", c.commit_time.format("%Y-%m-%d %H:%M"))
        } else {
            write!(
                f,
                "{} (at {})",
                &c.commit_id[0..8],
                c.commit_time.format("%Y-%m-%d %H:%M")
            )
        }
    }
}

struct ThreadSafeRepoReader<'i> {
    pub repo: Arc<Mutex<Repository>>,
    pub file_id_to_path: &'i [String],
    pub commit_index_to_commit_id: &'i [[u8; 20]],
}

impl<'i> RepoReader for ThreadSafeRepoReader<'i> {
    fn read_file_at_commit(
        &self,
        commit_id: CommitIndex,
        file_id: FileId,
    ) -> anyhow::Result<Option<(/*file path*/ String, /*content*/ String)>>
    {
        let file_path = self.file_id_to_path.get(file_id).unwrap();
        let commit =
            Oid::from_bytes(&self.commit_index_to_commit_id[commit_id])?;

        let repo = self.repo.lock().unwrap();
        let commit = repo.find_commit(commit)?;
        let tree = commit.tree()?;

        let entry = tree.get_path(Path::new(&file_path))?;
        let object = entry.to_object(&repo)?;
        if let Some(blob) = object.as_blob() {
            Ok(Some((
                file_path.to_owned(),
                String::from_utf8_lossy(blob.content()).to_string(),
            )))
        } else {
            Ok(None)
        }
    }
}
