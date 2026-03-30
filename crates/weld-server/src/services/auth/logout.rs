use crate::errors::ApiError;

use super::helpers;

/// Invalidate a refresh token by deleting it from the database.
pub async fn logout(pool: &sqlx::PgPool, refresh_token: &str) -> Result<(), ApiError> {
    let hashed = helpers::hash_refresh_token(refresh_token);
    sqlx::query!("DELETE FROM refresh_tokens WHERE token_hash = $1", hashed)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?;
    Ok(())
}
