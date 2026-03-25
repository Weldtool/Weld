use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Http error: {0}")]
    Http(String),
    #[error("Json error: {0}")]
    Json(String),
    #[error("Crypto error: {0}")]
    Crypto(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Internal server error: {0}")]
    InternalServer(String),
    #[error("Conflict error: {0}")]
    Conflict(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
}
