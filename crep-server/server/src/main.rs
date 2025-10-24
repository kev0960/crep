use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use axum::routing::{get, post};
use axum::serve;
use axum::Router;
use crep_indexer::index::git_index::GitIndex;
use git2::Repository;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::api::docs_json;
use crate::api::health::health;
use crate::api::search::search;

mod api;

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<Mutex<GitIndex>>,
    pub repo: Arc<Mutex<Repository>>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/search", post(search))
        .route("/docs.json", get(docs_json))
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let repo_path = PathBuf::from(std::env::var("CREP_REPO_PATH")?);
    let index_path = PathBuf::from(std::env::var("CREP_INDEX_PATH")?);

    info!("loading git index from {}", index_path.to_string_lossy());
    let index = GitIndex::load(&index_path)?;
    info!("loaded index with {} files", index.file_id_to_path.len());

    let repo = Repository::open(&*repo_path).unwrap();

    let state = AppState {
        index: Arc::new(Mutex::new(index)),
        repo: Arc::new(Mutex::new(repo)),
    };

    let app = router(state);
    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()?;

    info!("serving api at http://{addr}");

    let listener = TcpListener::bind(addr).await?;
    serve(listener, app).await?;

    Ok(())
}
