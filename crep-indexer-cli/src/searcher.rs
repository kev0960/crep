use std::path::Path;

use crep_indexer::index::git_index::GitIndex;
use crep_indexer::search::git_searcher::GitSearcher;
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

        let raw_results = match query {
            Query::Regex(regex) => self.searcher.regex_search(
                regex,
                Some(SearchOption {
                    max_num_to_find: Some(10),
                }),
            ),
            Query::RawString(key) => Ok(self.searcher.search(
                key,
                Some(SearchOption {
                    max_num_to_find: Some(10),
                }),
            )),
        };

        if raw_results.is_err() {
            anyhow::bail!("{raw_results:?}")
        }

        let raw_results = raw_results.unwrap();

        for result in raw_results {
            debug!("Checking {result:?}");

            for commit_id in &result.overlapped_commits {
                debug!("Checking {commit_id}");

                let (file_path, content) = self.read_file_at_commit(
                    result.file_id as usize,
                    commit_id as usize,
                )?;

                let search_result = SearchResult::new(
                    &result,
                    file_path,
                    &content.lines().collect::<Vec<&str>>(),
                )?;

                if let Some(search_result) = search_result {
                    search_results.push(search_result);

                    // Only add one case.
                    break;
                }
            }
        }

        Ok(search_results)
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
