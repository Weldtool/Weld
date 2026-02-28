use crate::models::enums::GlobaleRole;
use crate::models::enums::TeamRole;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: Uuid,
    email: String,
    display_name: String,
    avatar_url: Option<String>,
    global_role: GlobaleRole,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Team {
    id: Uuid,
    name: String,
    slug: String,
    owner_id: Uuid,
    created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamMembership {
    user_id: Uuid,
    team_id: Uuid,
    role: TeamRole,
    join_time: DateTime<Utc>,
}
