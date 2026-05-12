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
               short_id, github_issue_id, github_pr_id, github_repo,
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

pub async fn fetch_task_label_ids(pool: &PgPool, task_id: Uuid) -> Result<Vec<Uuid>, ApiError> {
    let rows = sqlx::query_scalar!(
        "SELECT label_id FROM task_labels WHERE task_id = $1",
        task_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
