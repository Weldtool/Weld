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
    // Emails are compared case-insensitively everywhere, so store one canonical
    // form and refuse a second account that differs only by case.
    let email = email.trim().to_lowercase();
    let taken = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE LOWER(email) = $1)",
        email
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if taken {
        return Err(ApiError::Conflict("Email is already registered".into()));
    }

    // hash password with Argon2id
    let salt =
        argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let password_hash = argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ApiError::Internal(format!("Password hashing failed: {e}")))?
        .to_string();

    let mut tx = pool.begin().await?;

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
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            ApiError::Conflict("Email is already registered".into())
        }
        _ => ApiError::Internal(format!("Database error: {e}")),
    })?;

    // Give every user a personal team — a private workspace separate from any
    // shared teams. Personal projects/board/chat reuse all team-scoped logic.
    let personal_slug = format!("personal-{}", user.id);
    let personal_team_id = sqlx::query_scalar!(
        r#"
        INSERT INTO teams (name, slug, owner_id, is_personal)
        VALUES ('Personal', $1, $2, TRUE)
        RETURNING id
        "#,
        personal_slug,
        user.id,
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'owner')",
        personal_team_id,
        user.id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

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
