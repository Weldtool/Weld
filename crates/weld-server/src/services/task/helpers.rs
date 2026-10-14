use crate::errors::ApiError;
use crate::services::project::helpers::require_project_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;
use weld_core::models::task::Task;

pub struct TaskWithLabels {
    pub task: Task,
    pub labels: Vec<Uuid>,
}

pub async fn fetch_active_task(pool: &PgPool, task_id: Uuid) -> Result<Task, ApiError> {
    sqlx::query_as!(
        Task,
        r#"
        SELECT id, project_id, column_id, title, description,
               assignee_id, reporter_id,
               priority as "priority: weld_core::models::enums::Priority",
               position, due_date, estimate_hours,
               short_id, github_issue_id, github_pr_id, github_repo, github_check_status,
               deleted_at, created_at, updated_at
        FROM tasks
        WHERE id = $1 AND deleted_at IS NULL
        "#,
        task_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Task not found".into()))
}

/// Rejects label ids that don't belong to `project_id`.
///
/// `task_labels` only has a foreign key to `labels`, so without this a task
/// could carry a label from another project — or another team entirely.
pub async fn validate_labels(
    pool: &PgPool,
    project_id: Uuid,
    label_ids: &[Uuid],
) -> Result<(), ApiError> {
    if label_ids.is_empty() {
        return Ok(());
    }
    let unique: std::collections::HashSet<&Uuid> = label_ids.iter().collect();
    let found = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM labels WHERE project_id = $1 AND id = ANY($2)"#,
        project_id,
        label_ids,
    )
    .fetch_one(pool)
    .await?;

    if found as usize != unique.len() {
        return Err(ApiError::Validation(
            "One or more labels do not belong to this project".into(),
        ));
    }
    Ok(())
}

/// Rejects an assignee who can't open the project.
///
/// Team membership alone isn't enough: Members and Viewers only see projects
/// they have been added to, and a card assigned to someone who can't open it
/// is invisible to the person meant to do it.
pub async fn validate_assignee(
    pool: &PgPool,
    project: &Project,
    assignee_id: Uuid,
) -> Result<(), ApiError> {
    require_project_team_role(pool, project, assignee_id, TeamRole::Viewer)
        .await
        .map_err(|e| match e {
            ApiError::Forbidden(_) | ApiError::NotFound(_) => {
                ApiError::Validation("Assignee must have access to this project".into())
            }
            other => other,
        })
}

pub async fn fetch_task_label_ids(pool: &PgPool, task_id: Uuid) -> Result<Vec<Uuid>, ApiError> {
    let rows = sqlx::query_scalar!(
        "SELECT label_id FROM task_labels WHERE task_id = $1",
        task_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
