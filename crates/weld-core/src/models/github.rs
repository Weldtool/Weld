use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConnection {
    pub id: Uuid,
    pub team_id: Uuid,
    pub installation_id: i64,
    pub access_token_encrypted: Vec<u8>,
    pub repos: Vec<String>, // Connected repositories
    pub webhook_secret: String,
    pub created_at: DateTime<Utc>,
}
