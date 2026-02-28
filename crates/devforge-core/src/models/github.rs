use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConnection {
    id: Uuid,
    team_id: Uuid,
    installation_id: i64,
    access_token_encrypted: Vec<u8>,
    repos: Vec<String>, // Connected repositories
    webhook_secret: String,
    created_at: DateTime<Utc>,
}
