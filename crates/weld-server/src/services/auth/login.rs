use crate::errors::ApiError;
use argon2::{Argon2, PasswordVerifier};

use super::helpers;

pub struct LoginResult {
    pub access_token: String,
    pub refresh_token: String,
}

/// Authenticate a user by email/password, return access + refresh tokens.
pub async fn login_user(
    pool: &sqlx::PgPool,
    email: &str,
    password: &str,
    jwt_secret: &str,
) -> Result<LoginResult, ApiError> {
    // Fetch user by email. Case-insensitive, matching how invites look people
    // up; oldest account wins should two legacy rows differ only by case.
    let user = sqlx::query!(
        r#"
        SELECT id, password_hash, role::text as "role!" FROM users
        WHERE LOWER(email) = LOWER($1)
        ORDER BY created_at ASC
        LIMIT 1"#,
        email.trim()
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?
    .ok_or_else(|| ApiError::Unauthorized("Invalid email or password".into()))?;

    // Verify password. Service accounts (the GitHub bot, study teammates) are
    // seeded with an empty hash so nobody can log in as them; that must look
    // like any other failed login rather than a 500 that reveals they exist.
    let parsed_hash = argon2::PasswordHash::new(&user.password_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid email or password".into()))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid email or password".into()))?;

    // Generate tokens
    let access_token = helpers::generate_access_token(user.id, &user.role, jwt_secret)?;
    let refresh_token = helpers::create_refresh_token(pool, user.id).await?;

    Ok(LoginResult {
        access_token,
        refresh_token,
    })
}
