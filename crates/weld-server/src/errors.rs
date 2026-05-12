use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum ApiError {
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        match &self {
            ApiError::Internal(_) => tracing::error!("{self}"),
            _ => tracing::warn!("{self}"),
        }

        let body = json!({"error": msg});
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => ApiError::NotFound("Resource not found".into()),
            _ => ApiError::Internal(err.to_string()),
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::Internal(format!("GitHub API request failed: {err}"))
    }
}

impl From<weld_core::errors::CoreError> for ApiError {
    fn from(err: weld_core::errors::CoreError) -> Self {
        match err {
            weld_core::errors::CoreError::NotFound(msg) => ApiError::NotFound(msg),
            weld_core::errors::CoreError::Unauthorized(msg) => ApiError::Unauthorized(msg),
            weld_core::errors::CoreError::Validation(msg) => ApiError::Validation(msg),
            _ => ApiError::Internal(err.to_string()),
        }
    }
}
