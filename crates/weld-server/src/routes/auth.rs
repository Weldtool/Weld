use crate::{errors::ApiError, state::AppState};
use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
