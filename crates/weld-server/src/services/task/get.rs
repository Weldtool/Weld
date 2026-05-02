use super::helpers::{TaskWithLabels, fetch_active_task, fetch_task_label_ids};
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub async fn get_task(
    pool: &PgPool,
    task_id: Uuid,
    caller_id: Uuid,
) -> Result<TaskWithLabels, ApiError> {
    let task = fetch_active_task(pool, task_id).await?;
    let project = fetch_active_project(pool, task.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Viewer).await?;
    let labels = fetch_task_label_ids(pool, task.id).await?;
    Ok(TaskWithLabels { task, labels })
}
