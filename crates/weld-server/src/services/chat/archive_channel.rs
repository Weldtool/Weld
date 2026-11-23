use super::helpers::{fetch_active_channel, require_channel_admin};
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn archive_channel(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
) -> Result<(), ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;
    require_channel_admin(pool, &channel, caller_id).await?;

    sqlx::query!(
        "UPDATE channels SET deleted_at = NOW(), updated_at = NOW()
         WHERE id = $1 AND deleted_at IS NULL",
        channel_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
