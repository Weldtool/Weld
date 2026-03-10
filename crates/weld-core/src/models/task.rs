use crate::models::enums::Priority;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid, // -> Project
    pub column_id: Uuid,  // -> Column
    pub title: String,
    pub description: Option<String>,  // Markdown
    pub assignee_id: Option<Uuid>,    // -> User
    pub reporter_id: Uuid,            // -> User
    pub priority: Priority,           // Critical, High, Medium, Low, None
    pub labels: Vec<Uuid>,            // -> Label
    pub github_issue_id: Option<i64>, // Linked GitHub issue number
    pub github_pr_id: Option<i64>,    // Linked GitHub PR number
    pub position: f64,                // Ordering within column (fractional indexing)
    pub due_date: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,           // e.g., "Backlog", "In Progress", "Done"
    pub position: f64,          // Column ordering
    pub wip_limit: Option<u32>, // Work-in-progress limit
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub color: String, // Hex color code
}
