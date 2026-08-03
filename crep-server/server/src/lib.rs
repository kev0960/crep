use crate::api::reindex;
use crate::config::LiveIndexConfig;
use crate::config::ServerConfig;
use crate::server_context::ServerContext;

use axum::Router;
use axum::routing::get;
use axum::routing::post;

pub mod api;
pub mod config;
pub mod indexer;
pub mod reindex_notify;
mod search;
pub mod server_context;

pub fn router(state: ServerContext, config: &ServerConfig) -> Router {
    let mut router = Router::new()
        .route("/api/health", get(api::health::health))
        .route("/api/search", post(api::search::search))
        .route("/docs.json", get(api::docs_json));

    if let Some(LiveIndexConfig::OnWebhookNotify) = &config.live_index_config {
        router = router.route("/webhook/reindex", post(reindex::reindex));
    }

    router.with_state(state)
}
