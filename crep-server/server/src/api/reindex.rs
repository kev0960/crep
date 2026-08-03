use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    reindex_notify::reindex_signal::ReindexSignal,
    server_context::ServerContext,
};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReindexRequest {
    pub commit_id: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReindexResponse {}

#[utoipa::path(
    post,
    path = "/webhook/reindex",
    request_body = ReindexRequest,
    responses(
        (status = 200, description = "Search results", body = ReindexResponse),
    ),
    tag = "reindex"
)]
pub async fn reindex(
    State(context): State<ServerContext>,
    Json(request): Json<ReindexRequest>,
) -> Json<ReindexResponse> {
    context.indexer.request_reindex(ReindexSignal {
        head_commit_id: request.commit_id,
    });

    Json(ReindexResponse {})
}
