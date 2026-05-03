use super::helpers::fetch_active_channel;
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub async fn archive_channel(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
) -> Result<(), ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;
    require_team_role(pool, channel.team_id, caller_id, TeamRole::Admin).await?;

    sqlx::query!(
        "UPDATE channels SET deleted_at = NOW(), updated_at = NOW()
         WHERE id = $1 AND deleted_at IS NULL",
        channel_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
