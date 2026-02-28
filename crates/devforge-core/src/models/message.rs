use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    id: Uuid,
    team_id: Uuid,
    project_id: Option<Uuid>, // None = team-wide channel
    name: String,
    is_direct: bool, // DM channel
    created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    id: Uuid,
    channel_id: Uuid,
    sender_id: Uuid,                // -> User
    content: String,                // Markdown / rich text JSON
    thread_parent_id: Option<Uuid>, // -> ChatMessage (for threads)
    edited_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}
