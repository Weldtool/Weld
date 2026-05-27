use super::helpers::{fetch_active_channel, require_channel_access};
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::message::ChatMessage;

pub struct MessagePage {
    pub messages: Vec<ChatMessage>,
    pub next_before: Option<Uuid>,
}

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 200;

/// Cursor-paginated list of top-level messages in a channel, newest first.
/// Replies (thread_parent_id IS NOT NULL) are excluded — fetch those via
/// `get_thread`.
pub async fn list_messages(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
    before_id: Option<Uuid>,
    limit: Option<i64>,
) -> Result<MessagePage, ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;
    require_channel_access(pool, &channel, caller_id).await?;

    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let before_created_at = if let Some(bid) = before_id {
        Some(
            sqlx::query_scalar!(
                "SELECT created_at FROM messages WHERE id = $1 AND channel_id = $2",
                bid,
                channel_id
            )
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| ApiError::Validation("Invalid pagination cursor".into()))?,
        )
    } else {
        None
    };

    let rows = sqlx::query_as!(
        ChatMessage,
        r#"
        SELECT id, channel_id, sender_id, content, thread_parent_id,
               mentions as "mentions!: Vec<Uuid>",
               task_refs as "task_refs!: Vec<Uuid>",
               metadata, edited_at, deleted_at, created_at
        FROM messages
        WHERE channel_id = $1
          AND deleted_at IS NULL
          AND thread_parent_id IS NULL
          AND ($2::timestamptz IS NULL OR created_at < $2)
        ORDER BY created_at DESC
        LIMIT $3
        "#,
        channel_id,
        before_created_at,
        limit,
    )
    .fetch_all(pool)
    .await?;

    // If we filled the page, the oldest message id is the cursor for the next page.
    let next_before = if rows.len() as i64 == limit {
        rows.last().map(|m| m.id)
    } else {
        None
    };

    Ok(MessagePage {
        messages: rows,
        next_before,
    })
}
