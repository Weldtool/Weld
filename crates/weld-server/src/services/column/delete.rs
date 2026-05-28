use super::helpers::fetch_column;
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use crate::services::task::helpers::{TaskWithLabels, fetch_task_label_ids};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::{Priority, TeamRole};
use weld_core::models::task::Task;

/// Delete a column, relocating any active tasks it holds.
///
/// When the column still has active tasks, they are moved to `move_to` (which
/// must be a different column in the same project). If `move_to` is omitted,
/// the project's "Backlog" column is used as a default; if there is no such
/// column, the caller must supply a target. Returns the tasks that were moved
/// so the caller can broadcast `task.moved` events.
pub async fn delete_column(
    pool: &PgPool,
    column_id: Uuid,
    caller_id: Uuid,
    move_to: Option<Uuid>,
) -> Result<Vec<TaskWithLabels>, ApiError> {
    let column = fetch_column(pool, column_id).await?;
    let project = fetch_active_project(pool, column.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Admin).await?;

    let active_tasks: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM tasks WHERE column_id = $1 AND deleted_at IS NULL",
        column_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    let mut moved: Vec<TaskWithLabels> = Vec::new();

    if active_tasks > 0 {
        let target_id = resolve_target_column(pool, &column, move_to).await?;
        let moved_tasks = relocate_tasks(pool, column_id, target_id).await?;
        for task in moved_tasks {
            let labels = fetch_task_label_ids(pool, task.id).await?;
            moved.push(TaskWithLabels { task, labels });
        }
    }

    sqlx::query!("DELETE FROM columns WHERE id = $1", column_id)
        .execute(pool)
        .await?;

    Ok(moved)
}

/// Picks the destination column for relocated tasks, validating the explicit
/// choice or falling back to the project's "Backlog" column.
async fn resolve_target_column(
    pool: &PgPool,
    column: &weld_core::models::task::Column,
    move_to: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    if let Some(target) = move_to {
        if target == column.id {
            return Err(ApiError::Validation(
                "Cannot move tasks into the column being deleted".into(),
            ));
        }
        let target_column = fetch_column(pool, target).await?;
        if target_column.project_id != column.project_id {
            return Err(ApiError::Validation(
                "Target column does not belong to this project".into(),
            ));
        }
        return Ok(target);
    }

    // Default: the project's Backlog column (case-insensitive), if any.
    let backlog: Option<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM columns
         WHERE project_id = $1 AND id <> $2 AND LOWER(name) = 'backlog'
         ORDER BY position ASC
         LIMIT 1",
        column.project_id,
        column.id,
    )
    .fetch_optional(pool)
    .await?;

    backlog.ok_or_else(|| {
        ApiError::Conflict(
            "Column has active tasks and no Backlog column exists; choose a target column".into(),
        )
    })
}

/// Moves every active task from `source` to `target`, appending them after the
/// target's existing tasks while preserving their relative order.
async fn relocate_tasks(pool: &PgPool, source: Uuid, target: Uuid) -> Result<Vec<Task>, ApiError> {
    let tasks = sqlx::query_as!(
        Task,
        r#"
        WITH base AS (
            SELECT COALESCE(MAX(position), 0.0) AS max_pos
            FROM tasks WHERE column_id = $2 AND deleted_at IS NULL
        ),
        ordered AS (
            SELECT id, ROW_NUMBER() OVER (ORDER BY position) AS rn
            FROM tasks WHERE column_id = $1 AND deleted_at IS NULL
        )
        UPDATE tasks t
        SET column_id = $2,
            position = base.max_pos + ordered.rn,
            updated_at = NOW()
        FROM ordered, base
        WHERE t.id = ordered.id
        RETURNING t.id, t.project_id, t.column_id, t.title, t.description,
                  t.assignee_id, t.reporter_id,
                  t.priority as "priority: Priority",
                  t.position, t.due_date, t.estimate_hours,
                  t.short_id, t.github_issue_id, t.github_pr_id, t.github_repo,
                  t.deleted_at, t.created_at, t.updated_at
        "#,
        source,
        target,
    )
    .fetch_all(pool)
    .await?;

    Ok(tasks)
}
