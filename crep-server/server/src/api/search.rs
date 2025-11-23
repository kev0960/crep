use std::time::Instant;

use axum::Json;
use axum::extract::State;
use crep_indexer::search::git_searcher::GitSearcher;
use crep_indexer::search::git_searcher::Query;
use crep_indexer::search::git_searcher::SearchOption;
use crep_indexer::search::result::search_result::SearchResult;
use crep_indexer::search::result::simple_repo_reader::SimpleRepoReader;
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
                results
                    .drain(
                        request.page * request.page_size
                            ..(request.page + 1) * request.page_size,
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

    let repo = context.repo.lock().unwrap();

    let reader = SimpleRepoReader {
        repo: &repo,
        file_id_to_path: &context.index.file_id_to_path,
        commit_index_to_commit_id: &context.index.commit_index_to_commit_id,
    };

    let mut hits = Vec::with_capacity(request.page_size);
    for result in results {
        let hit = match result {
            CacheResult::Hit(search_res) => {
                Ok(ShouldUpdateCache::Pass(Some(search_res)))
            }
            CacheResult::Miss(raw_res) => SearchResult::new(&reader, &raw_res)
                .map_err(|e| {
                    ApiError::internal("Unable to parse search result", e)
                })
                .map(ShouldUpdateCache::Update),
            _ => Ok(ShouldUpdateCache::Pass(None)),
        }?;

        hits.push(hit);
    }

    let results_to_update =
        hits.iter()
            .enumerate()
            .filter_map(|(index, res)| match res {
                ShouldUpdateCache::Update(res) => Some((
                    index + request.page * request.page_size,
                    res.clone(),
                )),
                ShouldUpdateCache::Pass(_) => None,
            });

    context
        .search_cache
        .put_search_results(&query, results_to_update);

    let results = hits
        .into_iter()
        .map(|c| {
            let result = match c {
                ShouldUpdateCache::Update(r) => r,
                ShouldUpdateCache::Pass(r) => r,
            };

            if let Some(result) = result {
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

enum ShouldUpdateCache {
    Update(Option<SearchResult>),
    Pass(Option<SearchResult>),
}
