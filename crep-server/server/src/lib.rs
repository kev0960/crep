use std::sync::Arc;
use std::sync::Mutex;

use axum::Router;
use axum::routing::{get, post};
use crep_indexer::index::git_index::GitIndex;
use git2::Repository;

pub mod api;

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<Mutex<GitIndex>>,
    pub repo: Arc<Mutex<Repository>>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(api::health::health))
        .route("/api/search", post(api::search::search))
        .route("/docs.json", get(api::docs_json))
        .with_state(state)
}
