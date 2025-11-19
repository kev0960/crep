use std::time::Instant;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use crep_indexer::index::git_index::GitIndex;
use crep_indexer::search::git_searcher::GitSearcher;
use crep_indexer::search::git_searcher::RawPerFileSearchResult;
use crep_indexer::search::git_searcher::SearchOption;
use crep_indexer::search::search_result::SearchResult;
use git2::Oid;
use git2::Repository;
use serde::Deserialize;
use serde::Serialize;
use tracing::info;
use utoipa::OpenApi;
use utoipa::ToSchema;

use crate::server_context::ServerContext;

#[derive(Default, Debug, Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    #[default]
    Plain,
    Regex,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub mode: SearchMode,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SearchResponse {
    pub results: Vec<SearchHit>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    pub file_path: String,
    pub first_match: MatchDetail,
    pub last_match: Option<MatchDetail>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct MatchDetail {
    pub commit_index: u32,
    pub commit_sha: String,
    pub commit_date: String,
    pub commit_summary: String,
    pub lines: Vec<LineMatch>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct LineMatch {
    pub line_number: usize,
    pub content: String,
    pub highlights: Vec<LineHighlight>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct LineHighlight {
    pub term: String,
    pub column: usize,
}

#[derive(OpenApi)]
#[openapi(
    paths(search),
    components(
        schemas(
            SearchRequest,
            SearchResponse,
            SearchHit,
            MatchDetail,
            LineMatch,
            LineHighlight,
            SearchMode,
            ErrorResponse
        )
    ),
    tags(
        (name = "search", description = "Git history search operations")
    )
)]
pub struct ApiDoc;

#[utoipa::path(
    post,
    path = "/api/search",
    request_body = SearchRequest,
    responses(
        (status = 200, description = "Search results", body = SearchResponse),
        (status = 400, description = "Invalid query", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "search"
)]
pub async fn search(
    State(context): State<ServerContext>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, ApiError> {
    let query = request.query.trim();
    if query.is_empty() {
        return Err(ApiError::bad_request("query must not be empty"));
    }

    let index = &context.index;

    let searcher = GitSearcher::new(index);
    let option = Some(SearchOption {
        max_num_to_find: request.limit,
    });

    let search_start = Instant::now();

    let raw_results = match request.mode {
        SearchMode::Plain => Ok(searcher.search(query, option)),
        SearchMode::Regex => searcher
            .regex_search(query, option)
            .map_err(|msg| ApiError::bad_request(&msg)),
    }?;

    info!(
        "Getting raw results took: {}ms",
        Instant::now().duration_since(search_start).as_millis()
    );

    let repo = context.repo.lock().unwrap();

    let mut hits = Vec::with_capacity(raw_results.len());
    for result in raw_results {
        let file_path = index.file_id_to_path[result.file_id as usize].clone();

        let (first, last) =
            resolve_first_and_last_match(&repo, index, &result)?;

        if let Some(first_match) = first {
            hits.push(SearchHit {
                file_path,
                first_match,
                last_match: last,
            });
        }

        if hits.len() >= 10 {
            break;
        }
    }

    Ok(Json(SearchResponse { results: hits }))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.to_owned(),
        }
    }

    fn internal(context: &str, err: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{context}: {err}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            message: self.message,
        });
        (self.status, body).into_response()
    }
}

fn resolve_first_and_last_match(
    repo: &Repository,
    index: &GitIndex,
    result: &RawPerFileSearchResult,
) -> Result<(Option<MatchDetail>, Option<MatchDetail>), ApiError> {
    let mut first = None;
    let mut last = None;

    for commit_idx in &result.overlapped_commits {
        if let Some(detail) = match_for_commit(repo, index, result, commit_idx)?
        {
            first = Some(detail);
            break;
        }
    }

    let Some(first_match) = first else {
        return Ok((None, None));
    };

    for commit_idx in result.overlapped_commits.iter().rev() {
        if commit_idx == first_match.commit_index {
            break;
        }

        if let Some(detail) = match_for_commit(repo, index, result, commit_idx)?
        {
            last = Some(detail);
            break;
        }
    }

    Ok((Some(first_match), last))
}

fn match_for_commit(
    repo: &Repository,
    index: &GitIndex,
    result: &RawPerFileSearchResult,
    commit_idx: u32,
) -> Result<Option<MatchDetail>, ApiError> {
    let file_id = result.file_id as usize;
    let commit_index = commit_idx as usize;

    let (file_path, content) =
        read_file_at_commit(repo, index, file_id, commit_index)?;

    let commit_result = SearchResult::new(
        &result.query,
        commit_index,
        file_path,
        &content.lines().collect::<Vec<&str>>(),
    )
    .map_err(|err| ApiError::internal("failed to render search result", err))?;

    let Some(search_result) = commit_result else {
        return Ok(None);
    };

    let metadata =
        commit_metadata(repo, index, commit_index).map_err(|err| {
            ApiError::internal("failed to read commit metadata", err)
        })?;

    let lines = search_result
        .lines
        .iter()
        .map(|(line_number, content)| LineMatch {
            line_number: line_number + 1,
            content: content.to_owned(),
            highlights: search_result
                .words_per_line
                .get(line_number)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|(term, column)| LineHighlight { term, column })
                .collect(),
        })
        .collect();

    Ok(Some(MatchDetail {
        commit_index: commit_idx,
        commit_sha: metadata.sha,
        commit_date: metadata.date,
        commit_summary: metadata.summary,
        lines,
    }))
}

fn read_file_at_commit<'a>(
    repo: &Repository,
    index: &'a GitIndex,
    file_id: usize,
    commit_index: usize,
) -> Result<(&'a str, String), ApiError> {
    let Some(commit_bytes) = index.commit_index_to_commit_id.get(commit_index)
    else {
        return Err(ApiError::internal(
            "commit index out of bounds",
            commit_index.to_string(),
        ));
    };

    let commit_oid = Oid::from_bytes(commit_bytes)
        .map_err(|err| ApiError::internal("failed to parse commit id", err))?;

    let commit = repo
        .find_commit(commit_oid)
        .map_err(|err| ApiError::internal("failed to find commit", err))?;
    let tree = commit
        .tree()
        .map_err(|err| ApiError::internal("failed to read tree", err))?;

    let file_path = &index.file_id_to_path[file_id];
    let entry =
        tree.get_path(std::path::Path::new(file_path))
            .map_err(|err| {
                ApiError::internal("failed to read file from tree", err)
            })?;

    let object = entry
        .to_object(repo)
        .map_err(|err| ApiError::internal("failed to read blob object", err))?;

    let blob = object.as_blob().ok_or_else(|| {
        ApiError::internal("path is not a file blob", file_path)
    })?;

    Ok((
        file_path,
        String::from_utf8_lossy(blob.content()).to_string(),
    ))
}

struct CommitMetadata {
    sha: String,
    date: String,
    summary: String,
}

fn commit_metadata(
    repo: &Repository,
    index: &GitIndex,
    commit_index: usize,
) -> anyhow::Result<CommitMetadata> {
    let commit_bytes = &index.commit_index_to_commit_id[commit_index];
    let oid = Oid::from_bytes(commit_bytes)?;
    let commit = repo.find_commit(oid)?;

    let sha = oid.to_string();

    let time = chrono::DateTime::from_timestamp_secs(commit.time().seconds())
        .ok_or_else(|| anyhow::anyhow!("invalid commit timestamp"))?;

    Ok(CommitMetadata {
        sha,
        date: time.to_rfc3339(),
        summary: commit
            .summary()
            .unwrap_or_else(|| commit.message().unwrap_or_default())
            .to_owned(),
    })
}
