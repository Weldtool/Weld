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

    // Consume the token in one statement (rotation). A separate SELECT then
    // DELETE let two concurrent requests both pass the check and both mint a
    // new pair from the same single-use token.
    let row = sqlx::query!(
        r#"
        WITH consumed AS (
            DELETE FROM refresh_tokens
            WHERE token_hash = $1 AND expires_at > NOW()
            RETURNING user_id
        )
        SELECT u.id as user_id, u.role::text as "role!"
        FROM consumed c
        JOIN users u ON u.id = c.user_id
        "#,
        hashed
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?
    .ok_or_else(|| ApiError::Unauthorized("Invalid or expired refresh token".into()))?;

    // Generate new tokens
    let access_token = helpers::generate_access_token(row.user_id, &row.role, jwt_secret)?;
    let new_refresh_token = helpers::create_refresh_token(pool, row.user_id).await?;

    Ok(RefreshResult {
        access_token,
        refresh_token: new_refresh_token,
    })
}
