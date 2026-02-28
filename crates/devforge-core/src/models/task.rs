use crate::models::enums::Priority;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    id: Uuid,
    project_id: Uuid, // -> Project
    column_id: Uuid,  // -> Column
    title: String,
    description: Option<String>,  // Markdown
    assignee_id: Option<Uuid>,    // -> User
    reporter_id: Uuid,            // -> User
    priority: Priority,           // Critical, High, Medium, Low, None
    labels: Vec<Uuid>,            // -> Label
    github_issue_id: Option<i64>, // Linked GitHub issue number
    github_pr_id: Option<i64>,    // Linked GitHub PR number
    position: f64,                // Ordering within column (fractional indexing)
    due_date: Option<DateTime<Utc>>,
    estimate_hours: Option<f32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    id: Uuid,
    project_id: Uuid,
    name: String,           // e.g., "Backlog", "In Progress", "Done"
    position: f64,          // Column ordering
    wip_limit: Option<u32>, // Work-in-progress limit
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    id: Uuid,
    project_id: Uuid,
    name: String,
    color: String, // Hex color code
}
