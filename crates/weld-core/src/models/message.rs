use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: Uuid,
    pub team_id: Uuid,
    pub project_id: Option<Uuid>, // None = team-wide channel
    pub name: String,
    pub is_direct: bool, // DM channel
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub sender_id: Uuid,                // -> User
    pub content: String,                // Markdown / rich text JSON
    pub thread_parent_id: Option<Uuid>, // -> ChatMessage (for threads)
    pub edited_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
