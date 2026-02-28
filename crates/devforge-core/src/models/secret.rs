use crate::models::enums::Environment;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    id: Uuid,
    project_id: Uuid,
    name: String,             // e.g., "STRIPE_API_KEY"
    encrypted_value: Vec<u8>, // AES-256-GCM encrypted
    nonce: Vec<u8>,           // Encryption nonce
    environment: Environment, // Development, Staging, Production
    created_by: Uuid,         // -> User
    last_rotated: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
