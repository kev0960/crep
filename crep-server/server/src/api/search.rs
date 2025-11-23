use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use axum::Json;
use axum::extract::State;
use crep_indexer::index::git_indexer::CommitIndex;
use crep_indexer::index::git_indexer::FileId;
use crep_indexer::search::git_searcher::GitSearcher;
use crep_indexer::search::git_searcher::Query;
use crep_indexer::search::git_searcher::SearchOption;
use crep_indexer::search::result::search_result::RepoReader;
use crep_indexer::search::result::search_result::SearchResult;
use git2::Oid;
use git2::Repository;
use serde::Deserialize;
use serde::Serialize;
use tracing::info;
use utoipa::OpenApi;
use utoipa::ToSchema;

use crate::api::error::ApiError;
use crate::api::error::ErrorResponse;
use crate::search::search::LineHighlight;
use crate::search::search::LineMatch;
use crate::search::search::MatchDetail;
use crate::search::search::SearchHit;
use crate::search::search_cache::CacheResult;
use crate::server_context::ServerContext;
use rayon::prelude::*;

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
    pub page: usize,

    #[serde(default)]
    pub page_size: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SearchResponse {
    pub results: Vec<Option<SearchHit>>,
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
    info!("Request {:?}", request);

    let query = request.query.trim();
    if query.is_empty() {
        return Err(ApiError::bad_request("query must not be empty"));
    }

    let index = &context.index;

    let searcher = GitSearcher::new(index);
    let option = Some(SearchOption {
        max_num_to_find: None,
    });

    let search_start = Instant::now();

    let query = match request.mode {
        SearchMode::Plain => Query::Plain(query.to_owned()),
        SearchMode::Regex => Query::Regex(query.to_owned()),
    };

    let results = context.search_cache.find(
        &query,
        request.page * request.page_size
            ..(request.page + 1) * request.page_size,
    );

    let results = match results {
        None => {
            let raw_results = searcher
                .search(&query, option)
                .map(|res| {
                    res.into_iter().map(CacheResult::Miss).collect::<Vec<_>>()
                })
                .map_err(ApiError::bad_request);

            if let Ok(raw_results) = &raw_results {
                context.search_cache.put_raw_result(
                    &query,
                    raw_results
                        .iter()
                        .map(|r| match r {
                            CacheResult::Miss(r) => r.clone(),
                            _ => panic!("Not possible"),
                        })
                        .collect(),
                );
            }

            raw_results.map(|mut results| {
                if request.page * request.page_size >= results.len() {
                    return vec![];
                }

                results
                    .drain(
                        request.page * request.page_size
                            ..std::cmp::min(
                                (request.page + 1) * request.page_size,
                                results.len(),
                            ),
                    )
                    .collect::<Vec<_>>()
            })
        }
        Some(results) => Ok(results),
    }?;

    info!(
        "Getting raw results took: {}ms, count: {}",
        Instant::now().duration_since(search_start).as_millis(),
        results.len()
    );

    let repo_pool = context.repo_pool.clone();
    let index_cloned = context.index.clone();

    let conversion_start = Instant::now();
    let result = tokio::task::spawn_blocking(move || {
        results
            .into_par_iter()
            .map_init(
                || ThreadSafeRepoReader {
                    repo: repo_pool
                        .repos
                        .get(rayon::current_thread_index().unwrap())
                        .unwrap()
                        .clone(),
                    file_id_to_path: &index_cloned.file_id_to_path,
                    commit_index_to_commit_id: &index_cloned
                        .commit_index_to_commit_id,
                },
                |reader, result| match result {
                    CacheResult::Hit(search_res) => {
                        Ok(SearchConversionResult {
                            result: Some(search_res),
                            ..Default::default()
                        })
                    }
                    CacheResult::Miss(raw_res) => {
                        let conversion_start = Instant::now();
                        SearchResult::new(reader, &raw_res)
                            .map_err(|e| {
                                ApiError::internal(
                                    "Unable to parse search result",
                                    e,
                                )
                            })
                            .map(|r| SearchConversionResult {
                                result: r,
                                should_update_cache: true,
                                duration: Some(
                                    Instant::now()
                                        .duration_since(conversion_start),
                                ),
                            })
                    }
                    _ => Ok(SearchConversionResult::default()),
                },
            )
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| ApiError::internal("Error during join", e))?
    .into_iter()
    .collect::<Result<Vec<SearchConversionResult>, _>>()?;

    info!(
        "To SearchResult conversion took {}ms",
        Instant::now().duration_since(conversion_start).as_millis()
    );

    let timings = result
        .iter()
        .filter_map(|c| c.duration.map(|c| c.as_millis()))
        .collect::<Vec<_>>();

    if !timings.is_empty() {
        info!(
            "Per each : {:.2}ms",
            timings.iter().copied().sum::<u128>() as f64 / timings.len() as f64
        );
    }

    let results_to_update =
        result.iter().enumerate().filter_map(|(index, res)| {
            match res.should_update_cache {
                true => Some((
                    index + request.page * request.page_size,
                    res.result.clone(),
                )),
                false => None,
            }
        });

    context
        .search_cache
        .put_search_results(&query, results_to_update);

    let repo = context.repo_pool.repos.first().unwrap().lock().unwrap();
    let results = result
        .into_iter()
        .map(|c| {
            if let Some(result) = c.result {
                SearchHit::from_search_result(
                    &repo,
                    &index.commit_index_to_commit_id,
                    result,
                )
                .map(Some)
                .map_err(|e| {
                    ApiError::internal(
                        "Unable to convert search result to response",
                        e,
                    )
                })
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(SearchResponse { results }))
}

#[derive(Default)]
struct SearchConversionResult {
    result: Option<SearchResult>,
    should_update_cache: bool,
    duration: Option<Duration>,
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
