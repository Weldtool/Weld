use crate::errors::ApiError;

use super::helpers;

pub struct RefreshResult {
    pub access_token: String,
    pub refresh_token: String,
}

/// Validate and rotate a refresh token, returning new access + refresh tokens.
pub async fn refresh_tokens(
    pool: &sqlx::PgPool,
    refresh_token: &str,
    jwt_secret: &str,
) -> Result<RefreshResult, ApiError> {
    let hashed = helpers::hash_refresh_token(refresh_token);

    let row = sqlx::query!(
        r#"
        SELECT rt.user_id, u.role::text as "role!"
        FROM refresh_tokens rt
        JOIN users u ON u.id = rt.user_id
        WHERE rt.token_hash = $1 AND rt.expires_at > NOW()
        "#,
        hashed
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?
    .ok_or_else(|| ApiError::Unauthorized("Invalid or expired refresh token".into()))?;

    // Delete old token (rotation)
    sqlx::query!("DELETE FROM refresh_tokens WHERE token_hash = $1", hashed)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?;

    // Generate new tokens
    let access_token = helpers::generate_access_token(row.user_id, &row.role, jwt_secret)?;
    let new_refresh_token = helpers::create_refresh_token(pool, row.user_id).await?;

    Ok(RefreshResult {
        access_token,
        refresh_token: new_refresh_token,
    })
}
