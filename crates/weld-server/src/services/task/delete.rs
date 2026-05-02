use super::helpers::fetch_active_task;
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub async fn delete_task(pool: &PgPool, task_id: Uuid, caller_id: Uuid) -> Result<(), ApiError> {
    let task = fetch_active_task(pool, task_id).await?;
    let project = fetch_active_project(pool, task.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    sqlx::query!(
        "UPDATE tasks SET deleted_at = NOW(), updated_at = NOW()
         WHERE id = $1 AND deleted_at IS NULL",
        task_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
