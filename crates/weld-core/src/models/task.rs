use crate::models::enums::Priority;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Labels associated with a task are stored in the `task_labels` join table
// and fetched separately rather than embedded on the Task struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub column_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub reporter_id: Uuid,
    pub priority: Priority,
    pub position: f64,
    pub due_date: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f32>,
    pub short_id: i64,
    pub github_issue_id: Option<i64>,
    pub github_pr_id: Option<i64>,
    pub github_repo: Option<String>,
    /// Latest CI conclusion for the linked PR, in GitHub's own vocabulary
    /// (`success`, `failure`, …) plus `pending`. `None` = no result seen.
    pub github_check_status: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub position: f64,
    pub wip_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
}
