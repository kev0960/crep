use utoipa::OpenApi;

use crate::api::error::ErrorResponse;
use crate::api::reindex::ReindexRequest;
use crate::api::reindex::ReindexResponse;
use crate::api::search::SearchMode;
use crate::api::search::SearchRequest;
use crate::api::search::SearchResponse;
use crate::search::search::LineHighlight;
use crate::search::search::LineMatch;
use crate::search::search::MatchDetail;
use crate::search::search::SearchHit;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::search::search,
        crate::api::reindex::reindex
    ),
    components(
        schemas(
            SearchRequest,
            SearchResponse,
            SearchHit,
            MatchDetail,
            LineMatch,
            LineHighlight,
            SearchMode,
            ErrorResponse,
            ReindexRequest,
            ReindexResponse
        )
    ),
    tags(
        (name = "search", description = "Git history search operations"),
        (name = "reindex", description = "Git history reindex operations")
    )
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_reindex_operation_and_schemas() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();

        assert!(document.pointer("/paths/~1webhook~1reindex/post").is_some());
        assert!(
            document
                .pointer("/components/schemas/ReindexRequest")
                .is_some()
        );
        assert!(
            document
                .pointer("/components/schemas/ReindexResponse")
                .is_some()
        );
    }
}
