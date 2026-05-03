use super::helpers::{fetch_active_channel, require_channel_access};
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

/// Update the caller's `last_read_at` for the channel to NOW().
/// For public channels, inserts the membership row if missing.
pub async fn mark_channel_read(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
) -> Result<(), ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;
    require_channel_access(pool, &channel, caller_id).await?;

    sqlx::query!(
        r#"
        INSERT INTO channel_memberships (channel_id, user_id, last_read_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (channel_id, user_id)
            DO UPDATE SET last_read_at = NOW()
        "#,
        channel_id,
        caller_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
