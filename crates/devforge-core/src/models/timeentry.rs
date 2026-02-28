use crate::models::enums::TimesheetStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    id: Uuid,
    user_id: Uuid,
    task_id: Option<Uuid>, // Can track time without a task
    project_id: Uuid,
    description: Option<String>,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>, // None = timer is running
    duration_seconds: i64,           // Calculated or manual
    is_manual: bool,                 // Manual entry vs timer
    created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timesheet {
    id: Uuid,
    user_id: Uuid,
    team_id: Uuid,
    period_start: DateTime<Utc>, // Week start (Monday)
    period_end: DateTime<Utc>,   // Week end (Sunday)
    status: TimesheetStatus,     // Draft, Submitted, Approved, Rejected
    submitted_at: Option<DateTime<Utc>>,
    reviewed_by: Option<Uuid>,
    reviewed_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}
