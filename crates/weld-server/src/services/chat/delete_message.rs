use super::helpers::{fetch_active_channel, fetch_active_message};
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub struct DeletedMessage {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub thread_parent_id: Option<Uuid>,
}

/// Soft-delete a message. Allowed for the author or a team Admin+.
/// Returns the message's channel_id and thread_parent_id so callers can
/// publish realtime events without re-querying.
pub async fn delete_message(
    pool: &PgPool,
    message_id: Uuid,
    caller_id: Uuid,
) -> Result<DeletedMessage, ApiError> {
    let existing = fetch_active_message(pool, message_id).await?;
    let channel = fetch_active_channel(pool, existing.channel_id).await?;

    if existing.sender_id != caller_id {
        require_team_role(pool, channel.team_id, caller_id, TeamRole::Admin).await?;
    } else {
        require_team_role(pool, channel.team_id, caller_id, TeamRole::Viewer).await?;
    }

    sqlx::query!(
        "UPDATE messages SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        message_id
    )
    .execute(pool)
    .await?;
    Ok(DeletedMessage {
        id: existing.id,
        channel_id: existing.channel_id,
        thread_parent_id: existing.thread_parent_id,
    })
}
