use super::helpers::{fetch_active_channel, fetch_active_message};
use super::parse_mentions::resolve_mentions;
use super::parse_task_refs::resolve_task_refs;
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::message::ChatMessage;

pub async fn edit_message(
    pool: &PgPool,
    message_id: Uuid,
    caller_id: Uuid,
    new_content: String,
) -> Result<ChatMessage, ApiError> {
    let existing = fetch_active_message(pool, message_id).await?;
    let channel = fetch_active_channel(pool, existing.channel_id).await?;

    if existing.sender_id != caller_id {
        return Err(ApiError::Forbidden(
            "Only the author can edit a message".into(),
        ));
    }
    // ensure caller is still on the team and can access the channel
    super::helpers::require_channel_post_access(pool, &channel, caller_id).await?;

    let content = new_content.trim().to_string();
    if content.is_empty() {
        return Err(ApiError::Validation(
            "Message content cannot be empty".into(),
        ));
    }
    if content.len() > 16_000 {
        return Err(ApiError::Validation("Message content too long".into()));
    }

    let mentions = resolve_mentions(pool, channel.team_id, &content).await?;
    let task_refs = resolve_task_refs(pool, channel.team_id, &content).await?;

    let updated = sqlx::query_as!(
        ChatMessage,
        r#"
        UPDATE messages
        SET content = $1, mentions = $2, task_refs = $4, edited_at = NOW()
        WHERE id = $3 AND deleted_at IS NULL
        RETURNING id, channel_id, sender_id, content, thread_parent_id,
                  mentions as "mentions!: Vec<Uuid>",
                  task_refs as "task_refs!: Vec<Uuid>",
                  metadata, edited_at, deleted_at, created_at
        "#,
        content,
        &mentions,
        message_id,
        &task_refs,
    )
    .fetch_one(pool)
    .await?;

    Ok(updated)
}
