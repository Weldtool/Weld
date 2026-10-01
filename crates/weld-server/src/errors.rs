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
    #[error("Too many requests: {0}")]
    TooManyRequests(String),
    /// A dependency is down or not configured. Unlike `Internal`, the message
    /// is written for the user and is safe to show them.
    #[error("Service unavailable: {0}")]
    Unavailable(String),
    /// Something went wrong on our side. The message is for the logs only: it
    /// routinely carries database, GitHub or OpenAI error text, so the client is
    /// sent a generic body instead.
    #[error("Internal server error: {0}")]
    Internal(String),
}

const INTERNAL_ERROR_BODY: &str = "Internal server error";

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.as_str()),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.as_str()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.as_str()),
            ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg.as_str()),
            ApiError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg.as_str()),
            ApiError::Unavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.as_str()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERROR_BODY),
        };

        match &self {
            ApiError::Internal(_) | ApiError::Unavailable(_) => tracing::error!("{self}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn body_of(err: ApiError) -> (StatusCode, serde_json::Value) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn internal_errors_never_reach_the_client() {
        let (status, body) = body_of(ApiError::Internal(
            "Database error: relation \"users\" does not exist".into(),
        ))
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"], "Internal server error");
    }

    #[tokio::test]
    async fn user_facing_errors_keep_their_message() {
        let (status, body) = body_of(ApiError::Validation("Name cannot be empty".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "Name cannot be empty");

        let (status, body) =
            body_of(ApiError::Unavailable("The assistant is unavailable".into())).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "The assistant is unavailable");
    }
}
