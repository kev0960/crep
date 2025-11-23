use chrono::DateTime;
use crep_indexer::search::result::search_result::SearchResult;
use crep_indexer::search::result::single_commit_search_result::SingleCommitSearchResult;
use git2::Oid;
use git2::Repository;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    pub file_path: String,
    pub first_match: MatchDetail,
    pub last_match: Option<MatchDetail>,
}

impl SearchHit {
    pub fn from_search_result(
        repo: &Repository,
        commit_index_to_commit_id: &[[u8; 20]],
        s: SearchResult,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            file_path: s.file_path,
            first_match: MatchDetail::from_single_commit_result(
                repo,
                commit_index_to_commit_id,
                s.first_match,
            )?,
            last_match: match s.last_match {
                Some(last) => Some(MatchDetail::from_single_commit_result(
                    repo,
                    commit_index_to_commit_id,
                    last,
                )?),
                _ => None,
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct MatchDetail {
    pub commit_sha: String,
    pub commit_date: String,
    pub commit_summary: String,
    pub lines: Vec<LineMatch>,
}

impl MatchDetail {
    fn from_single_commit_result(
        repo: &Repository,
        commit_index_to_commit_id: &[[u8; 20]],
        result: SingleCommitSearchResult,
    ) -> anyhow::Result<Self> {
        let commit_id =
            Oid::from_bytes(&commit_index_to_commit_id[result.commit_id])?;

        let commit = repo.find_commit(commit_id)?;
        Ok(Self {
            commit_sha: commit_id.to_string(),
            commit_date: DateTime::from_timestamp_secs(commit.time().seconds())
                .ok_or_else(|| anyhow::anyhow!("invalid commit timestamp"))?
                .to_rfc3339(),
            commit_summary: commit.summary().unwrap_or_default().to_owned(),
            lines: LineMatch::new(&result),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct LineMatch {
    pub line_number: usize,
    pub content: String,
    pub highlights: Vec<LineHighlight>,
}

impl LineMatch {
    fn new(s: &SingleCommitSearchResult) -> Vec<Self> {
        s.lines
            .iter()
            .map(|(k, v)| {
                let highlights = match s.words_per_line.get(k) {
                    Some(words_in_line) => words_in_line
                        .iter()
                        .map(|(word, col)| LineHighlight {
                            term: word.to_owned(),
                            column: *col,
                        })
                        .collect::<Vec<_>>(),
                    None => vec![],
                };

                Self {
                    line_number: *k,
                    content: v.to_owned(),
                    highlights,
                }
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct LineHighlight {
    pub term: String,
    pub column: usize,
}
