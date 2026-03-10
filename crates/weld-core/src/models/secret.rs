use crate::models::enums::Environment;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,             // e.g., "STRIPE_API_KEY"
    pub encrypted_value: Vec<u8>, // AES-256-GCM encrypted
    pub nonce: Vec<u8>,           // Encryption nonce
    pub environment: Environment, // Development, Staging, Production
    pub created_by: Uuid,         // -> User
    pub last_rotated: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
