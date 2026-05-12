use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::Priority;
use weld_core::models::task::Task;

use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use crate::services::task::helpers::{TaskWithLabels, fetch_active_task, fetch_task_label_ids};

/// Links a task to a GitHub issue and/or PR. Requires Member+.
pub async fn link_task(
    pool: &PgPool,
    task_id: Uuid,
    caller_id: Uuid,
    repo: &str,
    issue_number: Option<i64>,
    pr_number: Option<i64>,
) -> Result<TaskWithLabels, ApiError> {
    let task = fetch_active_task(pool, task_id).await?;
    let project = fetch_active_project(pool, task.project_id).await?;
    require_project_team_role(
        pool,
        &project,
        caller_id,
        weld_core::models::enums::TeamRole::Member,
    )
    .await?;

    let updated = sqlx::query_as!(
        Task,
        r#"
        UPDATE tasks
        SET github_repo = $2, github_issue_id = $3, github_pr_id = $4, updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING id, project_id, column_id, title, description,
                  assignee_id, reporter_id,
                  priority as "priority: Priority",
                  position, due_date, estimate_hours,
                  short_id, github_issue_id, github_pr_id, github_repo,
                  deleted_at, created_at, updated_at
        "#,
        task_id,
        repo,
        issue_number,
        pr_number,
    )
    .fetch_one(pool)
    .await?;

    let labels = fetch_task_label_ids(pool, task_id).await?;
    Ok(TaskWithLabels {
        task: updated,
        labels,
    })
}

/// Clears the GitHub link fields on a task. Requires Member+.
pub async fn unlink_task(
    pool: &PgPool,
    task_id: Uuid,
    caller_id: Uuid,
) -> Result<TaskWithLabels, ApiError> {
    let task = fetch_active_task(pool, task_id).await?;
    let project = fetch_active_project(pool, task.project_id).await?;
    require_project_team_role(
        pool,
        &project,
        caller_id,
        weld_core::models::enums::TeamRole::Member,
    )
    .await?;

    let updated = sqlx::query_as!(
        Task,
        r#"
        UPDATE tasks
        SET github_repo = NULL, github_issue_id = NULL, github_pr_id = NULL, updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING id, project_id, column_id, title, description,
                  assignee_id, reporter_id,
                  priority as "priority: Priority",
                  position, due_date, estimate_hours,
                  short_id, github_issue_id, github_pr_id, github_repo,
                  deleted_at, created_at, updated_at
        "#,
        task_id,
    )
    .fetch_one(pool)
    .await?;

    let labels = fetch_task_label_ids(pool, task_id).await?;
    Ok(TaskWithLabels {
        task: updated,
        labels,
    })
}
