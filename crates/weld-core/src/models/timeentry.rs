use crate::models::enums::TimesheetStatus;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub task_id: Option<Uuid>, // Can track time without a task
    pub project_id: Uuid,
    pub description: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>, // None = timer is running
    pub duration_seconds: i64,           // Calculated or manual
    pub is_manual: bool,                 // Manual entry vs timer
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timesheet {
    pub id: Uuid,
    pub user_id: Uuid,
    pub team_id: Uuid,
    pub period_start: NaiveDate, // Week start (Monday)
    pub period_end: NaiveDate,   // Week end (Sunday)
    pub status: TimesheetStatus, // Draft, Submitted, Approved, Rejected
    pub submitted_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}
