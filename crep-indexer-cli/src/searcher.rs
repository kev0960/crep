use std::fmt;
use std::path::Path;
use std::time::Instant;

use chrono::DateTime;
use chrono::Utc;
use crep_indexer::index::git_index::GitIndex;
use crep_indexer::search::git_searcher::GitSearcher;
use crep_indexer::search::git_searcher::Query;
use crep_indexer::search::git_searcher::SearchOption;
use crep_indexer::search::result::search_result::SearchResult;
use crep_indexer::search::result::simple_repo_reader::SimpleRepoReader;
use git2::Oid;
use git2::Repository;
use log::debug;
use log::info;

pub struct Searcher<'a> {
    repo: Repository,
    index: &'a GitIndex,
    searcher: GitSearcher<'a>,
}

impl<'a> Searcher<'a> {
    pub fn new(index: &'a GitIndex, path: &str) -> Self {
        Self {
            repo: Repository::open(Path::new(path)).unwrap(),
            index,
            searcher: GitSearcher::new(index),
        }
    }

    pub fn handle_query(
        &mut self,
        query: &Query,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let mut search_results = vec![];

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

        let mut raw_result_times = vec![Instant::now()];
        let reader = SimpleRepoReader {
            repo: &self.repo,
            file_id_to_path: &self.index.file_id_to_path,
            commit_index_to_commit_id: &self.index.commit_index_to_commit_id,
        };

        for result in raw_results {
            debug!(
                "Checking {result:?} at {}",
                self.index.file_id_to_path[result.file_id as usize]
            );

            if let Some(result) = SearchResult::new(&reader, &result)? {
                search_results.push(result);
            }

            if search_results.len() >= 10 {
                break;
            }

            raw_result_times.push(Instant::now());
        }

        show_raw_result_timing(&raw_result_times);

        Ok(search_results)
    }

    pub fn get_commit_info(
        &self,
        commit_index: usize,
    ) -> anyhow::Result<CommitInfo> {
        let commit_id = Oid::from_bytes(
            &self.index.commit_index_to_commit_id[commit_index],
        )?;

        let commit = self.repo.find_commit(commit_id)?;

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

fn show_raw_result_timing(timings: &[Instant]) {
    let mut gaps = vec![];
    for i in 0..timings.len() - 1 {
        gaps.push(timings[i + 1].duration_since(timings[i]).as_secs_f64());
    }

    let total = gaps.iter().sum::<f64>();
    info!("Avg result : {}ms", total / (gaps.len() as f64) * 1000.);
    info!("Total : {}ms", total * 1000.);
}
