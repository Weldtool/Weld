use crate::{errors::ApiError, state::AppState};
use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::http::HeaderMap;
use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use uuid::Uuid;
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
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub iat: i64,
    pub exp: i64,
}

pub async fn register(
    State(state): State<AppState>,
    Json(input): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), ApiError> {
    // 1. Validate input
    input
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // 2. Hash password with Argon2id
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(input.password.as_bytes(), &salt)
        .map_err(|e| ApiError::Internal(format!("Password hashing failed: {e}")))?
        .to_string();

    // 3. Insert into database
    let user = sqlx::query!(
        r#"
        INSERT INTO users (email, password_hash, display_name)
        VALUES ($1, $2, $3)
        RETURNING id, email, display_name, avatar_url, role::text as "role!", email_verified, created_at, updated_at
        "#,
        input.email,
        password_hash,
        input.display_name
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            ApiError::Conflict("Email is already registered".into())
        }
        _ => ApiError::Internal(format!("Database error: {e}")),
    })?;
    let response = RegisterResponse {
        id: user.id,
        email: user.email,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
        role: user.role,
        email_verified: user.email_verified,
        created_at: user.created_at,
        updated_at: user.updated_at,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginRequest>,
) -> Result<(StatusCode, HeaderMap, Json<LoginResponse>), ApiError> {
    // 1. Fetch user by email
    let user = sqlx::query!(
        r#"
        SELECT id, password_hash, role::text as "role!"
        FROM users
        WHERE email = $1
        "#,
        input.email
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?
    .ok_or_else(|| ApiError::Unauthorized("Invalid email or password".into()))?;

    // 2. Verify password
    let parsed_hash = argon2::PasswordHash::new(&user.password_hash)
        .map_err(|e| ApiError::Internal(format!("Invalid password hash: {e}")))?;
    let argon2 = Argon2::default();
    argon2
        .verify_password(input.password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid email or password".into()))?;

    // 3. Generate JWT token
    let claims = Claims {
        sub: user.id.to_string(),
        role: user.role,
        iat: Utc::now().timestamp(),
        exp: (Utc::now() + chrono::Duration::minutes(15)).timestamp(),
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_ref()),
    )
    .map_err(|e| ApiError::Internal(format!("Token generation failed: {e}")))?;

    // 4. Generate Refresh token
    let refresh_token = Uuid::new_v4().to_string();
    let hashed_refresh_token = format!("{:x}", sha2::Sha256::digest(refresh_token.as_bytes()));
    sqlx::query!(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
        VALUES ($1, $2, $3)
        "#,
        user.id,
        hashed_refresh_token,
        Utc::now() + chrono::Duration::days(7)
    )
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?;

    let mut headers = HeaderMap::new();
    let cookie = format!(
        "refresh_token={}; HttpOnly; SameSite=Strict; Path=/api/v1/auth; Max-Age=604800",
        refresh_token
    );
    headers.insert(
        axum::http::header::SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("Failed to set cookie header".into()))?,
    );

    Ok((
        StatusCode::OK,
        headers,
        Json(LoginResponse {
            access_token: token,
            token_type: "Bearer".into(),
        }),
    ))
}
