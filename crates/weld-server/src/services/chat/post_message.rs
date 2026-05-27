use super::helpers::{fetch_active_channel, fetch_active_message, require_channel_post_access};
use super::parse_mentions::resolve_mentions;
use super::parse_task_refs::resolve_task_refs;
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::message::ChatMessage;

pub struct PostMessageInput {
    pub content: String,
    pub thread_parent_id: Option<Uuid>,
}

pub async fn post_message(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
    input: PostMessageInput,
) -> Result<ChatMessage, ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;
    require_channel_post_access(pool, &channel, caller_id).await?;

    let content = input.content.trim().to_string();
    if content.is_empty() {
        return Err(ApiError::Validation(
            "Message content cannot be empty".into(),
        ));
    }
    if content.len() > 16_000 {
        return Err(ApiError::Validation("Message content too long".into()));
    }

    if let Some(parent_id) = input.thread_parent_id {
        let parent = fetch_active_message(pool, parent_id).await?;
        if parent.channel_id != channel_id {
            return Err(ApiError::Validation(
                "Thread parent is not in this channel".into(),
            ));
        }
        if parent.thread_parent_id.is_some() {
            return Err(ApiError::Validation(
                "Cannot reply to a thread reply".into(),
            ));
        }
    }

    let mentions = resolve_mentions(pool, channel.team_id, &content).await?;
    let task_refs = resolve_task_refs(pool, channel.team_id, &content).await?;

    let message = sqlx::query_as!(
        ChatMessage,
        r#"
        INSERT INTO messages (channel_id, sender_id, content, thread_parent_id, mentions, task_refs)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, channel_id, sender_id, content, thread_parent_id,
                  mentions as "mentions!: Vec<Uuid>",
                  task_refs as "task_refs!: Vec<Uuid>",
                  metadata, edited_at, deleted_at, created_at
        "#,
        channel_id,
        caller_id,
        content,
        input.thread_parent_id,
        &mentions,
        &task_refs,
    )
    .fetch_one(pool)
    .await?;

    Ok(message)
}
