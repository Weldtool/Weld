use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
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

/// Rejects an assignee who isn't on the project's team.
pub async fn validate_assignee(
    pool: &PgPool,
    team_id: Uuid,
    assignee_id: Uuid,
) -> Result<(), ApiError> {
    let on_team = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM team_memberships
                        WHERE team_id = $1 AND user_id = $2)",
        team_id,
        assignee_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);

    if !on_team {
        return Err(ApiError::Validation(
            "Assignee must be a member of the team".into(),
        ));
    }
    Ok(())
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
