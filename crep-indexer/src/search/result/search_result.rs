use serde::Deserialize;
use serde::Serialize;

use crate::index::git_indexer::CommitIndex;
use crate::index::git_indexer::FileId;
use crate::index::not_committed_indexer::NotCommitedFilesIndexer;
use crate::search::git_searcher::MatchedQuery;
use crate::search::git_searcher::RawPerFileSearchResult;
use crate::search::result::single_commit_search_result::SingleCommitSearchResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub first_match: SingleCommitSearchResult,
    pub last_match: Option<SingleCommitSearchResult>,
}

impl SearchResult {
    pub fn new<Reader: RepoReader>(
        reader: &Reader,
        not_committed_indexer: &Option<&NotCommitedFilesIndexer>,
        result: &RawPerFileSearchResult,
    ) -> anyhow::Result<Option<Self>> {
        let mut file_path = None;
        let mut first = None;
        let mut last = None;

        for commit_id in &result.overlapped_commits {
            if let (file_path_read, Some(first_match)) =
                SearchResult::get_search_result_at_commit(
                    reader,
                    &result.query,
                    commit_id as CommitIndex,
                    result.file_id as FileId,
                )?
            {
                first = Some(first_match);
                file_path = Some(file_path_read);
                break;
            }
        }

        if first.is_none() {
            return Ok(None);
        }

        let first_commit_id = first.as_ref().unwrap().commit_id;
        for commit_id in result.overlapped_commits.iter().rev() {
            if commit_id as usize <= first_commit_id {
                break;
            }

            if let (_, Some(last_match)) =
                SearchResult::get_search_result_at_commit(
                    reader,
                    &result.query,
                    commit_id as CommitIndex,
                    result.file_id as FileId,
                )?
            {
                last = Some(last_match);
                break;
            }
        }

        Ok(Some(Self {
            file_path: file_path.unwrap(),
            first_match: first.unwrap(),
            last_match: last,
        }))
    }

    fn get_search_result_at_commit<Reader: RepoReader>(
        reader: &Reader,
        query: &MatchedQuery,
        commit_id: CommitIndex,
        file_id: FileId,
    ) -> anyhow::Result<(String, Option<SingleCommitSearchResult>)> {
        let search_result = reader
            .read_file_at_commit(commit_id as CommitIndex, file_id as FileId)?;

        if search_result.is_none() {
            return Ok(("".to_owned(), None));
        }

        let (file_path, content) = search_result.unwrap();

        Ok((
            file_path,
            SingleCommitSearchResult::new(
                query,
                commit_id,
                &content.lines().collect::<Vec<&str>>(),
            )?,
        ))
    }
}

pub trait RepoReader {
    fn read_file_at_commit(
        &self,
        commit_id: CommitIndex,
        file_id: FileId,
    ) -> anyhow::Result<Option<(/*file path*/ String, /*content*/ String)>>;
}
