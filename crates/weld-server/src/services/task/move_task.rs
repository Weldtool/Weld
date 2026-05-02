use super::helpers::{TaskWithLabels, fetch_active_task, fetch_task_label_ids};
use crate::errors::ApiError;
use crate::services::column::helpers::{POSITION_PRECISION_FLOOR, fetch_column};
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::{Priority, TeamRole};
use weld_core::models::task::Task;

/// Move a task to a new column / position. The caller supplies the desired
/// `position` directly (computed client-side as midpoint between neighbours,
/// `last + 1.0` for end-of-column, etc.). The server validates that the new
/// column belongs to the same project and that there is enough numerical
/// headroom between adjacent tasks; if not, it returns 409 so the client can
/// rebalance and retry.
pub async fn move_task(
    pool: &PgPool,
    task_id: Uuid,
    caller_id: Uuid,
    new_column_id: Uuid,
    new_position: f64,
) -> Result<TaskWithLabels, ApiError> {
    if !new_position.is_finite() {
        return Err(ApiError::Validation(
            "position must be a finite number".into(),
        ));
    }

    let task = fetch_active_task(pool, task_id).await?;
    let project = fetch_active_project(pool, task.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    let target_column = fetch_column(pool, new_column_id).await?;
    if target_column.project_id != task.project_id {
        return Err(ApiError::Validation(
            "Target column does not belong to this project".into(),
        ));
    }

    // Look at the closest neighbour above and below the requested position in
    // the destination column, skipping the task itself if it's already there.
    let neighbour_above: Option<f64> = sqlx::query_scalar!(
        r#"
        SELECT position FROM tasks
        WHERE column_id = $1 AND deleted_at IS NULL
          AND id <> $2 AND position < $3
        ORDER BY position DESC
        LIMIT 1
        "#,
        new_column_id,
        task_id,
        new_position
    )
    .fetch_optional(pool)
    .await?;

    let neighbour_below: Option<f64> = sqlx::query_scalar!(
        r#"
        SELECT position FROM tasks
        WHERE column_id = $1 AND deleted_at IS NULL
          AND id <> $2 AND position > $3
        ORDER BY position ASC
        LIMIT 1
        "#,
        new_column_id,
        task_id,
        new_position
    )
    .fetch_optional(pool)
    .await?;

    if let (Some(above), Some(below)) = (neighbour_above, neighbour_below)
        && (below - above) < POSITION_PRECISION_FLOOR
    {
        return Err(ApiError::Conflict("rebalance required".into()));
    }

    let updated = sqlx::query_as!(
        Task,
        r#"
        UPDATE tasks
        SET column_id = $1, position = $2, updated_at = NOW()
        WHERE id = $3 AND deleted_at IS NULL
        RETURNING id, project_id, column_id, title, description,
                  assignee_id, reporter_id,
                  priority as "priority: Priority",
                  position, due_date, estimate_hours,
                  github_issue_id, github_pr_id,
                  deleted_at, created_at, updated_at
        "#,
        new_column_id,
        new_position,
        task_id
    )
    .fetch_one(pool)
    .await?;

    let labels = fetch_task_label_ids(pool, task_id).await?;
    Ok(TaskWithLabels {
        task: updated,
        labels,
    })
}
