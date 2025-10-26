use crate::server_context::ServerContext;

use axum::Router;
use axum::routing::get;
use axum::routing::post;

pub mod api;
pub mod config;
pub mod server_context;

pub fn router(state: ServerContext) -> Router {
    Router::new()
        .route("/api/health", get(api::health::health))
        .route("/api/search", post(api::search::search))
        .route("/docs.json", get(api::docs_json))
        .with_state(state)
}
