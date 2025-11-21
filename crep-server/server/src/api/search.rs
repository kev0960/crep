use std::time::Instant;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use chrono::DateTime;
use crep_indexer::search::git_searcher::GitSearcher;
use crep_indexer::search::git_searcher::SearchOption;
use crep_indexer::search::result::search_result::SearchResult;
use crep_indexer::search::result::simple_repo_reader::SimpleRepoReader;
use crep_indexer::search::result::single_commit_search_result::SingleCommitSearchResult;
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

impl SearchHit {
    fn from_search_result(
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

    let reader = SimpleRepoReader {
        repo: &repo,
        file_id_to_path: &context.index.file_id_to_path,
        commit_index_to_commit_id: &context.index.commit_index_to_commit_id,
    };

    let mut hits = Vec::with_capacity(raw_results.len());
    for result in raw_results {
        let result = SearchResult::new(&reader, &result);
        if result.is_err() {
            return Err(ApiError::internal(
                "Unable to read",
                result.err().unwrap(),
            ));
        }

        if let Some(r) = result.unwrap() {
            let result = SearchHit::from_search_result(
                &repo,
                &context.index.commit_index_to_commit_id,
                r,
            );

            if result.is_err() {
                return Err(ApiError::internal(
                    "Unable to parse search result",
                    result.err().unwrap(),
                ));
            }

            hits.push(result.unwrap());
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
