use crate::errors::ApiError;
use argon2::PasswordHasher;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct RegisteredUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Register a new user with Argon2id password hashing.
pub async fn register_user(
    pool: &sqlx::PgPool,
    email: &str,
    password: &str,
    display_name: &str,
) -> Result<RegisteredUser, ApiError> {
    // hash password with Argon2id
    let salt =
        argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let password_hash = argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ApiError::Internal(format!("Password hashing failed: {e}")))?
        .to_string();

    // Insert into database
    let user = sqlx::query!(
        r#"
        INSERT INTO users (email, password_hash, display_name)
        VALUES ($1, $2, $3)
        RETURNING id, email, display_name, avatar_url, role::text as "role!", email_verified, created_at, updated_at
        "#,
        email,
        password_hash,
        display_name
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            ApiError::Conflict("Email is already registered".into())
        }
        _ => ApiError::Internal(format!("Database error: {e}")),
    })?;

    Ok(RegisteredUser {
        id: user.id,
        email: user.email,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
        role: user.role,
        email_verified: user.email_verified,
        created_at: user.created_at,
        updated_at: user.updated_at,
    })
}
