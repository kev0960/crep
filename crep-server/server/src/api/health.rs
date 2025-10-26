use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[utoipa::path(get, path = "/api/health", responses(
    (status = 200, description = "Health check ok", body = HealthResponse)
))]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
