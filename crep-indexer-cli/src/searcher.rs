use std::path::Path;

use crep_indexer::index::git_index::GitIndex;
use crep_indexer::search::git_searcher::GitSearcher;
use crep_indexer::search::git_searcher::RawPerFileSearchResult;
use crep_indexer::search::git_searcher::SearchOption;
use crep_indexer::search::search_result::SearchResult;
use git2::Oid;
use git2::Repository;
use log::debug;

pub struct Searcher<'a> {
    repo: Repository,
    index: &'a GitIndex,
    searcher: GitSearcher<'a>,
}

#[derive(Debug)]
pub enum Query {
    Regex(String),
    RawString(String),
}

#[derive(Debug)]
pub struct FirstAndLastFound {
    pub first: SearchResult,
    pub last: Option<SearchResult>,
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
    ) -> anyhow::Result<Vec<FirstAndLastFound>> {
        let mut search_results = vec![];

        let raw_results = match query {
            Query::Regex(regex) => self.searcher.regex_search(
                regex,
                Some(SearchOption {
                    max_num_to_find: None,
                }),
            ),
            Query::RawString(key) => Ok(self.searcher.search(
                key,
                Some(SearchOption {
                    max_num_to_find: None,
                }),
            )),
        };

        if raw_results.is_err() {
            anyhow::bail!("{raw_results:?}")
        }

        let raw_results = raw_results.unwrap();

        for result in raw_results {
            debug!(
                "Checking {result:?} at {}",
                self.index.file_id_to_path[result.file_id as usize]
            );

            let mut first = None;
            let mut last = None;

            for commit_id in &result.overlapped_commits {
                debug!("Checking {commit_id}");

                let search_result =
                    self.get_search_result_at_commit(commit_id, &result)?;

                if search_result.is_some() {
                    first = search_result;
                    break;
                }
            }

            if first.is_none() {
                continue;
            }

            let first_commit_id = first.as_ref().unwrap().commit_id;
            for commit_id in result.overlapped_commits.iter().rev() {
                let search_result =
                    self.get_search_result_at_commit(commit_id, &result)?;

                if first_commit_id == commit_id as usize {
                    break;
                }

                if search_result.is_some() {
                    last = search_result;
                    break;
                }
            }

            search_results.push(FirstAndLastFound {
                first: first.unwrap(),
                last,
            })
        }

        Ok(search_results)
    }

    fn get_search_result_at_commit(
        &self,
        commit_id: u32,
        result: &RawPerFileSearchResult,
    ) -> anyhow::Result<Option<SearchResult>> {
        let (file_path, content) = self
            .read_file_at_commit(result.file_id as usize, commit_id as usize)?;

        SearchResult::new(
            &result.query,
            commit_id as usize,
            file_path,
            &content.lines().collect::<Vec<&str>>(),
        )
    }

    fn read_file_at_commit(
        &self,
        file_id: usize,
        commit_index: usize,
    ) -> anyhow::Result<(&str, String)> {
        let commit_id = Oid::from_bytes(
            &self.index.commit_index_to_commit_id[commit_index],
        )?;

        let commit = self.repo.find_commit(commit_id)?;
        let tree = commit.tree()?;

        let file_path = &self.index.file_id_to_path[file_id];
        debug!("File path : {file_path} {}", hex::encode(commit_id));

        let entry = tree.get_path(Path::new(file_path))?;

        let object = entry.to_object(&self.repo)?;
        if let Some(blob) = object.as_blob() {
            Ok((
                file_path,
                String::from_utf8_lossy(blob.content()).to_string(),
            ))
        } else {
            anyhow::bail!("Path is not a blob file {file_path}");
        }
    }
}
