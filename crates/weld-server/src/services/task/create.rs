use super::helpers::{TaskWithLabels, validate_assignee, validate_labels};
use crate::errors::ApiError;
use crate::services::column::helpers::fetch_column;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::{Priority, TeamRole};
use weld_core::models::task::Task;

pub struct CreateTaskInput {
    pub title: String,
    pub description: Option<String>,
    pub column_id: Uuid,
    pub assignee_id: Option<Uuid>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<Uuid>>,
    pub due_date: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f32>,
}

pub async fn create_task(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
    input: CreateTaskInput,
) -> Result<TaskWithLabels, ApiError> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(ApiError::Validation("Task title cannot be empty".into()));
    }

    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    let column = fetch_column(pool, input.column_id).await?;
    if column.project_id != project_id {
        return Err(ApiError::Validation(
            "Column does not belong to this project".into(),
        ));
    }

    if let Some(assignee_id) = input.assignee_id {
        validate_assignee(pool, &project, assignee_id).await?;
    }
    if let Some(labels) = &input.labels {
        validate_labels(pool, project_id, labels).await?;
    }

    let next_position: Option<f64> = sqlx::query_scalar!(
        "SELECT MAX(position) FROM tasks WHERE column_id = $1 AND deleted_at IS NULL",
        input.column_id
    )
    .fetch_one(pool)
    .await?;
    let position = next_position.map(|p| p + 1.0).unwrap_or(1.0);

    let priority = input.priority.unwrap_or(Priority::None);

    let mut tx = pool.begin().await?;

    let task = sqlx::query_as!(
        Task,
        r#"
        INSERT INTO tasks
          (project_id, column_id, title, description, assignee_id, reporter_id,
           priority, position, due_date, estimate_hours)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id, project_id, column_id, title, description,
                  assignee_id, reporter_id,
                  priority as "priority: Priority",
                  position, due_date, estimate_hours,
                  short_id, github_issue_id, github_pr_id, github_repo, github_check_status,
                  deleted_at, created_at, updated_at
        "#,
        project_id,
        input.column_id,
        title,
        input.description.as_deref(),
        input.assignee_id,
        caller_id,
        priority as Priority,
        position,
        input.due_date,
        input.estimate_hours,
    )
    .fetch_one(&mut *tx)
    .await?;

    let mut label_ids: Vec<Uuid> = Vec::new();
    if let Some(labels) = input.labels {
        for label_id in labels {
            sqlx::query!(
                "INSERT INTO task_labels (task_id, label_id) VALUES ($1, $2)
                 ON CONFLICT DO NOTHING",
                task.id,
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
            label_ids.push(label_id);
        }
    }

    tx.commit().await?;

    Ok(TaskWithLabels {
        task,
        labels: label_ids,
    })
}
