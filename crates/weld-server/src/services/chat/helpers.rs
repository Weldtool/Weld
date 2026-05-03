use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::message::{Channel, ChatMessage};

pub async fn fetch_active_channel(pool: &PgPool, id: Uuid) -> Result<Channel, ApiError> {
    sqlx::query_as!(
        Channel,
        r#"
        SELECT id, team_id, project_id, name, topic, is_direct, is_private,
               created_by, created_at, updated_at, deleted_at
        FROM channels
        WHERE id = $1 AND deleted_at IS NULL
        "#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Channel not found".into()))
}

pub async fn fetch_active_message(pool: &PgPool, id: Uuid) -> Result<ChatMessage, ApiError> {
    sqlx::query_as!(
        ChatMessage,
        r#"
        SELECT id, channel_id, sender_id, content, thread_parent_id,
               mentions as "mentions!: Vec<Uuid>",
               edited_at, deleted_at, created_at
        FROM messages
        WHERE id = $1 AND deleted_at IS NULL
        "#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Message not found".into()))
}

pub async fn is_channel_member(
    pool: &PgPool,
    channel_id: Uuid,
    user_id: Uuid,
) -> Result<bool, ApiError> {
    let exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM channel_memberships
                       WHERE channel_id = $1 AND user_id = $2)",
        channel_id,
        user_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    Ok(exists)
}

/// Verify the user can read/post in this channel.
///
/// - For public channels: user must be a Viewer+ on the team.
/// - For private channels and DMs: user must additionally be in
///   `channel_memberships`.
///
/// Returns the user's team role on success.
pub async fn require_channel_access(
    pool: &PgPool,
    channel: &Channel,
    user_id: Uuid,
) -> Result<TeamRole, ApiError> {
    let role = require_team_role(pool, channel.team_id, user_id, TeamRole::Viewer).await?;

    if channel.is_private || channel.is_direct {
        let member = is_channel_member(pool, channel.id, user_id).await?;
        if !member {
            return Err(ApiError::Forbidden("Not a member of this channel".into()));
        }
    }

    Ok(role)
}

/// Verify the user can post (no Viewer-only). Same channel access rules.
pub async fn require_channel_post_access(
    pool: &PgPool,
    channel: &Channel,
    user_id: Uuid,
) -> Result<TeamRole, ApiError> {
    let role = require_channel_access(pool, channel, user_id).await?;
    if matches!(role, TeamRole::Viewer) {
        return Err(ApiError::Forbidden("Viewers cannot post messages".into()));
    }
    Ok(role)
}

#[derive(Debug, Clone)]
pub struct LastMessage {
    pub channel_id: Uuid,
    pub created_at: DateTime<Utc>,
}
