use super::helpers::{TaskWithLabels, fetch_active_task, validate_assignee, validate_labels};
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::{Priority, TeamRole};
use weld_core::models::task::Task;

#[derive(Default)]
pub struct UpdateTaskInput {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub assignee_id: Option<Option<Uuid>>,
    pub priority: Option<Priority>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub estimate_hours: Option<Option<f32>>,
    pub labels: Option<Vec<Uuid>>,
}

pub async fn update_task(
    pool: &PgPool,
    task_id: Uuid,
    caller_id: Uuid,
    input: UpdateTaskInput,
) -> Result<TaskWithLabels, ApiError> {
    let task = fetch_active_task(pool, task_id).await?;
    let project = fetch_active_project(pool, task.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    if let Some(t) = &input.title
        && t.trim().is_empty()
    {
        return Err(ApiError::Validation("Task title cannot be empty".into()));
    }

    if let Some(Some(assignee_id)) = input.assignee_id {
        validate_assignee(pool, project.team_id, assignee_id).await?;
    }
    if let Some(labels) = &input.labels {
        validate_labels(pool, task.project_id, labels).await?;
    }

    let new_title = input
        .title
        .as_ref()
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|| task.title.clone());
    let new_description = match &input.description {
        Some(v) => v.clone(),
        None => task.description.clone(),
    };
    let new_assignee = match input.assignee_id {
        Some(v) => v,
        None => task.assignee_id,
    };
    let new_priority = input.priority.unwrap_or(task.priority.clone());
    let new_due_date = match input.due_date {
        Some(v) => v,
        None => task.due_date,
    };
    let new_estimate = match input.estimate_hours {
        Some(v) => v,
        None => task.estimate_hours,
    };

    let mut tx = pool.begin().await?;

    let updated = sqlx::query_as!(
        Task,
        r#"
        UPDATE tasks
        SET title = $1, description = $2, assignee_id = $3, priority = $4,
            due_date = $5, estimate_hours = $6, updated_at = NOW()
        WHERE id = $7 AND deleted_at IS NULL
        RETURNING id, project_id, column_id, title, description,
                  assignee_id, reporter_id,
                  priority as "priority: Priority",
                  position, due_date, estimate_hours,
                  short_id, github_issue_id, github_pr_id, github_repo, github_check_status,
                  deleted_at, created_at, updated_at
        "#,
        new_title,
        new_description,
        new_assignee,
        new_priority as Priority,
        new_due_date,
        new_estimate,
        task_id
    )
    .fetch_one(&mut *tx)
    .await?;

    let label_ids = if let Some(labels) = input.labels {
        sqlx::query!("DELETE FROM task_labels WHERE task_id = $1", task_id)
            .execute(&mut *tx)
            .await?;
        for label_id in &labels {
            sqlx::query!(
                "INSERT INTO task_labels (task_id, label_id) VALUES ($1, $2)
                 ON CONFLICT DO NOTHING",
                task_id,
                label_id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
                    ApiError::Validation("One or more labels do not exist".into())
                }
                _ => ApiError::from(e),
            })?;
        }
        labels
    } else {
        fetch_task_label_ids_in_tx(&mut tx, task_id).await?
    };

    tx.commit().await?;

    Ok(TaskWithLabels {
        task: updated,
        labels: label_ids,
    })
}

async fn fetch_task_label_ids_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Uuid,
) -> Result<Vec<Uuid>, ApiError> {
    let rows = sqlx::query_scalar!(
        "SELECT label_id FROM task_labels WHERE task_id = $1",
        task_id
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows)
}
