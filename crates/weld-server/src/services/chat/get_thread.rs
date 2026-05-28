use super::helpers::{fetch_active_channel, fetch_active_message, require_channel_access};
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::message::ChatMessage;

pub struct ThreadView {
    pub parent: ChatMessage,
    pub replies: Vec<ChatMessage>,
}

/// Fetch a top-level message and all of its replies, oldest first.
pub async fn get_thread(
    pool: &PgPool,
    parent_id: Uuid,
    caller_id: Uuid,
) -> Result<ThreadView, ApiError> {
    let parent = fetch_active_message(pool, parent_id).await?;
    if parent.thread_parent_id.is_some() {
        return Err(ApiError::Validation("Not a top-level message".into()));
    }
    let channel = fetch_active_channel(pool, parent.channel_id).await?;
    require_channel_access(pool, &channel, caller_id).await?;

    let replies = sqlx::query_as!(
        ChatMessage,
        r#"
        SELECT id, channel_id, sender_id, content, thread_parent_id,
               mentions as "mentions!: Vec<Uuid>",
               task_refs as "task_refs!: Vec<Uuid>",
               metadata, edited_at, deleted_at, created_at
        FROM messages
        WHERE thread_parent_id = $1 AND deleted_at IS NULL
        ORDER BY created_at ASC
        "#,
        parent_id
    )
    .fetch_all(pool)
    .await?;

    Ok(ThreadView { parent, replies })
}
