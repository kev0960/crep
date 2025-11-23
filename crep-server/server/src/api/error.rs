use std::borrow::Cow;

use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn bad_request<'a>(message: impl Into<Cow<'a, str>>) -> Self {
        let message: Cow<'a, str> = message.into();
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into_owned(),
        }
    }

    pub fn internal(context: &str, err: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{context}: {err}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            message: self.message,
        });
        (self.status, body).into_response()
    }
}
