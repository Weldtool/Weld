use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub team_id: Uuid, // -> Team
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub github_repo: Option<String>, // "owner/repo"
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
