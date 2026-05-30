//! PetTracker seed fixture: the frozen team state every study participant is
//! provisioned from. The canonical content lives in
//! `research-paper/pettracker_seed.json`; the copy embedded here
//! (`study/pettracker_seed.json`) is the runtime source of truth — keep them in
//! sync. Only the fields the provisioner needs are modeled; serde ignores the
//! rest (e.g. the GitHub fixtures, which load with the GitHub-mock work).

use serde::Deserialize;
use uuid::Uuid;

/// Embedded at compile time so provisioning never depends on the working dir.
pub const SEED_JSON: &str = include_str!("../../../study/pettracker_seed.json");

#[derive(Debug, Deserialize)]
pub struct Seed {
    pub project: SeedProject,
    pub columns: Vec<SeedColumn>,
    pub labels: Vec<SeedLabel>,
    pub cards: Vec<SeedCard>,
    pub channels: Vec<SeedChannel>,
    pub messages: Vec<SeedMessage>,
}

#[derive(Debug, Deserialize)]
pub struct SeedProject {
    pub name: String,
    pub description: Option<String>,
    pub github_repo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SeedColumn {
    pub id: String,
    pub name: String,
    pub position: f64,
}

#[derive(Debug, Deserialize)]
pub struct SeedLabel {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
pub struct SeedCard {
    pub id: String,
    pub column_id: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub assignee_id: Option<String>,
    pub priority: Option<String>,
    pub position: f64,
    pub github_issue: Option<i64>,
    pub github_pr: Option<i64>,
    pub created_offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct SeedChannel {
    pub id: String,
    pub name: String,
    pub project_scoped: bool,
}

#[derive(Debug, Deserialize)]
pub struct SeedMessage {
    pub channel_id: String,
    pub sender_id: String,
    pub content: String,
    pub created_offset: i64,
}

impl Seed {
    /// Parse the embedded fixture. Errors are programmer errors (bad fixture).
    pub fn load() -> Result<Self, serde_json::Error> {
        serde_json::from_str(SEED_JSON)
    }
}

// --- Fixed-UUID synthetic teammates (seeded in the study migration) ---

const TEAMMATE_SARAH: Uuid = Uuid::from_u128(0xa1);
const TEAMMATE_ALEX: Uuid = Uuid::from_u128(0xa2);
const TEAMMATE_PRIYA: Uuid = Uuid::from_u128(0xa3);
const TEAMMATE_MARCUS: Uuid = Uuid::from_u128(0xa4);

/// Default author for content with no explicit owner (e.g. channel creator,
/// reporter of an unassigned card): the tech lead, Alex.
pub const DEFAULT_AUTHOR: Uuid = TEAMMATE_ALEX;

/// Map a fixture teammate id (`user_sarah`, ...) to its fixed user UUID.
pub fn teammate_uuid(symbolic: &str) -> Option<Uuid> {
    match symbolic {
        "user_sarah" => Some(TEAMMATE_SARAH),
        "user_alex" => Some(TEAMMATE_ALEX),
        "user_priya" => Some(TEAMMATE_PRIYA),
        "user_marcus" => Some(TEAMMATE_MARCUS),
        _ => None,
    }
}

/// All teammate UUIDs (added to the cloned team/project/channels per participant).
pub const TEAMMATES: [Uuid; 4] = [
    TEAMMATE_SARAH,
    TEAMMATE_ALEX,
    TEAMMATE_PRIYA,
    TEAMMATE_MARCUS,
];

/// Map a fixture priority (`"High"`, `null`, ...) to the DB `priority` enum value.
pub fn priority_value(priority: &Option<String>) -> String {
    match priority {
        Some(p) => p.to_lowercase(),
        None => "none".to_string(),
    }
}
