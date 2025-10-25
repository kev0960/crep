use axum::Json;
use utoipa::OpenApi;

pub mod health;
pub mod search;

pub use health::health;
pub use search::{ApiDoc, search};

pub async fn docs_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
