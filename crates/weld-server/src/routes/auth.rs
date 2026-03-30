use crate::errors::ApiError;
use crate::services::auth;
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(length(min = 12, message = "Password must be at least 12 characters long"))]
    pub password: String,
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub display_name: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub id: uuid::Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub email_verified: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(input): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), ApiError> {
    input
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let user = auth::register_user(
        &state.db,
        &input.email,
        &input.password,
        &input.display_name,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
            role: user.role,
            email_verified: user.email_verified,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginRequest>,
) -> Result<(StatusCode, HeaderMap, Json<LoginResponse>), ApiError> {
    let result =
        auth::login_user(&state.db, &input.email, &input.password, &state.jwt_secret).await?;

    let mut headers = HeaderMap::new();
    let cookie =
        auth::helpers::build_refresh_token_cookie(&result.refresh_token, state.secure_cookies);
    headers.insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("Failed to set cookie header".into()))?,
    );

    Ok((
        StatusCode::OK,
        headers,
        Json(LoginResponse {
            access_token: result.access_token,
            token_type: "Bearer".into(),
        }),
    ))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let refresh_token = auth::helpers::extract_refresh_token(&headers)?;
    auth::logout(&state.db, &refresh_token).await?;
    Ok(StatusCode::OK)
}

pub async fn refresh_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, HeaderMap, Json<LoginResponse>), ApiError> {
    let token = auth::helpers::extract_refresh_token(&headers)?;
    let result = auth::refresh_tokens(&state.db, &token, &state.jwt_secret).await?;

    let mut response_headers = HeaderMap::new();
    let cookie =
        auth::helpers::build_refresh_token_cookie(&result.refresh_token, state.secure_cookies);
    response_headers.insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("Failed to set cookie header".into()))?,
    );

    Ok((
        StatusCode::OK,
        response_headers,
        Json(LoginResponse {
            access_token: result.access_token,
            token_type: "Bearer".into(),
        }),
    ))
}
