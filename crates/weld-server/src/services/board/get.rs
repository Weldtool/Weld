use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;
use weld_core::models::enums::{Priority, TeamRole};
use weld_core::models::task::{Column, Label, Task};

pub struct BoardState {
    pub columns: Vec<Column>,
    pub tasks: Vec<Task>,
    pub task_labels: HashMap<Uuid, Vec<Uuid>>,
    pub labels: Vec<Label>,
}

pub async fn get_board_state(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<BoardState, ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Viewer).await?;

    let columns = sqlx::query_as!(
        Column,
        r#"
        SELECT id, project_id, name, position, wip_limit, created_at, updated_at
        FROM columns
        WHERE project_id = $1
        ORDER BY position ASC
        "#,
        project_id
    )
    .fetch_all(pool)
    .await?;

    let tasks = sqlx::query_as!(
        Task,
        r#"
        SELECT id, project_id, column_id, title, description,
               assignee_id, reporter_id,
               priority as "priority: Priority",
               position, due_date, estimate_hours,
               short_id, github_issue_id, github_pr_id, github_repo,
               deleted_at, created_at, updated_at
        FROM tasks
        WHERE project_id = $1 AND deleted_at IS NULL
        ORDER BY column_id, position ASC
        "#,
        project_id
    )
    .fetch_all(pool)
    .await?;

    let task_label_rows = sqlx::query!(
        r#"
        SELECT tl.task_id, tl.label_id
        FROM task_labels tl
        JOIN tasks t ON t.id = tl.task_id
        WHERE t.project_id = $1 AND t.deleted_at IS NULL
        "#,
        project_id
    )
    .fetch_all(pool)
    .await?;

    let mut task_labels: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for row in task_label_rows {
        task_labels
            .entry(row.task_id)
            .or_default()
            .push(row.label_id);
    }

    let labels = sqlx::query_as!(
        Label,
        r#"
        SELECT id, project_id, name, color, created_at
        FROM labels
        WHERE project_id = $1
        ORDER BY name ASC
        "#,
        project_id
    )
    .fetch_all(pool)
    .await?;

    Ok(BoardState {
        columns,
        tasks,
        task_labels,
        labels,
    })
}
