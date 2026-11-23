use axum::Json;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
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

/// A message a user can act on for `status`, for when there is nothing more
/// specific that is safe to show them.
pub fn status_message(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 | 415 | 422 => {
            "Some of the information sent was missing or invalid. Please check it and try again."
        }
        401 => "Your session has expired. Please sign in again.",
        403 => "You don't have permission to do that.",
        404 => "That couldn't be found. It may have been moved or deleted.",
        405 => "That action isn't supported.",
        408 => "The request took too long. Please try again.",
        409 => "That conflicts with a change someone else just made. Please refresh and try again.",
        413 => "That is too large to send.",
        429 => "Too many attempts. Please wait a moment and try again.",
        503 => "The service is temporarily unavailable. Please try again shortly.",
        _ => "Something went wrong on our side. Please try again.",
    }
}

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
            ApiError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                status_message(StatusCode::INTERNAL_SERVER_ERROR),
            ),
        };

        match &self {
            ApiError::Internal(_) | ApiError::Unavailable(_) => tracing::error!("{self}"),
            _ => tracing::warn!("{self}"),
        }

        let body = json!({"error": msg});
        (status, Json(body)).into_response()
    }
}

/// Response middleware: gives every error response the `{"error": ...}` shape
/// the clients read, with a message written for the user.
///
/// `ApiError` already does this, but errors produced before a handler runs do
/// not go through it — axum's extractor rejections ("Failed to deserialize the
/// JSON body into the target type: missing field `email` at line 1 column 20"),
/// unknown routes, wrong methods, oversized bodies. Those arrive as plain text
/// that names our internal types and fields, or as an empty body. The original
/// text is logged for debugging and replaced with a generic message.
pub async fn normalize_error_responses(resp: Response) -> Response {
    let status = resp.status();
    let is_json = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if !(status.is_client_error() || status.is_server_error()) || is_json {
        return resp;
    }

    let (mut parts, body) = resp.into_parts();
    let detail = axum::body::to_bytes(body, 4096).await.unwrap_or_default();
    tracing::warn!(
        %status,
        detail = %String::from_utf8_lossy(&detail),
        "Replaced a non-JSON error response"
    );

    let body = json!({ "error": status_message(status) }).to_string();
    parts.headers.remove(CONTENT_LENGTH);
    parts
        .headers
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Response::from_parts(parts, axum::body::Body::from(body))
}

/// Shows the messages written on the `#[validate(...)]` attributes, without
/// the field names and codes that `ValidationErrors`' `Display` adds.
impl From<validator::ValidationErrors> for ApiError {
    fn from(errs: validator::ValidationErrors) -> Self {
        let mut messages: Vec<String> = errs
            .field_errors()
            .values()
            .flat_map(|errors| errors.iter())
            .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
            .collect();
        messages.sort();
        messages.dedup();
        if messages.is_empty() {
            ApiError::Validation(status_message(StatusCode::BAD_REQUEST).into())
        } else {
            ApiError::Validation(messages.join(" "))
        }
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
        assert_eq!(
            body["error"],
            "Something went wrong on our side. Please try again."
        );
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

    #[tokio::test]
    async fn plain_text_rejections_are_replaced() {
        let resp = (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Failed to deserialize the JSON body into the target type: missing field `email`",
        )
            .into_response();
        let resp = normalize_error_responses(resp).await;
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let message = body["error"].as_str().unwrap();
        assert!(!message.contains("email"), "{message}");
        assert_eq!(message, status_message(StatusCode::UNPROCESSABLE_ENTITY));
    }

    #[tokio::test]
    async fn json_errors_and_successes_pass_through() {
        let (status, body) =
            body_of(ApiError::Conflict("Email is already registered".into())).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "Email is already registered");

        let ok = normalize_error_responses((StatusCode::OK, "hello").into_response()).await;
        let bytes = axum::body::to_bytes(ok.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"hello");
    }

    #[test]
    fn validation_errors_show_only_the_written_messages() {
        use validator::Validate;

        #[derive(Validate)]
        struct Form {
            #[validate(length(min = 3, message = "Name is too short"))]
            name: String,
        }

        let err: ApiError = Form { name: "a".into() }.validate().unwrap_err().into();
        match err {
            ApiError::Validation(msg) => assert_eq!(msg, "Name is too short"),
            other => panic!("unexpected {other:?}"),
        }
    }
}
