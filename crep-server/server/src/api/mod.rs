use axum::Json;
use utoipa::OpenApi;

mod doc;
pub mod error;
pub mod health;
pub mod reindex;
pub mod search;

pub use doc::ApiDoc;
pub use health::health;
pub use search::search;

pub async fn docs_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
