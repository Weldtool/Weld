use std::sync::Arc;

use sqlx::PgPool;
use weld_core::models::enums::Priority;
use weld_core::models::task::Task;

use crate::errors::ApiError;
use crate::realtime::Hub;
use crate::realtime::broadcast::publish_project_event;
use crate::routes::tasks::TaskResponse;
use crate::services::task::helpers::fetch_task_label_ids;

/// Called by the `issues` webhook handler.  If the issue is closed and the
/// project is configured to auto-close on issue closed, moves the linked task
/// to the project's Done column and broadcasts a `task.moved` WS event.
#[tracing::instrument(skip(db, hub))]
pub async fn on_issue_event(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    issue_number: i64,
    action: &str,
) -> Result<(), ApiError> {
    if action != "closed" {
        return Ok(());
    }

    // Find tasks linked to this issue
    let tasks = sqlx::query_as!(
        Task,
        r#"
        SELECT t.id, t.project_id, t.column_id, t.title, t.description,
               t.assignee_id, t.reporter_id,
               t.priority as "priority: Priority",
               t.position, t.due_date, t.estimate_hours,
               t.short_id, t.github_issue_id, t.github_pr_id, t.github_repo,
               t.deleted_at, t.created_at, t.updated_at
        FROM tasks t
        WHERE t.github_issue_id = $1
          AND t.github_repo = $2
          AND t.deleted_at IS NULL
        "#,
        issue_number,
        repo,
    )
    .fetch_all(db)
    .await?;

    for task in tasks {
        move_to_role_column_if_configured(db, hub, task, PrCloseOutcome::Merged).await?;
    }

    Ok(())
}

/// Called by the `pull_request` webhook handler. When a linked PR closes, moves
/// the task: a merged PR sends it to the Done column, while a PR closed without
/// merging sends it back to the In Progress column. Both are gated on the
/// project's auto-move setting.
#[tracing::instrument(skip(db, hub))]
pub async fn on_pr_event(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    pr_number: i64,
    action: &str,
    merged: bool,
) -> Result<(), ApiError> {
    if action != "closed" {
        return Ok(());
    }

    let outcome = if merged {
        PrCloseOutcome::Merged
    } else {
        PrCloseOutcome::ClosedUnmerged
    };

    let tasks = sqlx::query_as!(
        Task,
        r#"
        SELECT t.id, t.project_id, t.column_id, t.title, t.description,
               t.assignee_id, t.reporter_id,
               t.priority as "priority: Priority",
               t.position, t.due_date, t.estimate_hours,
               t.short_id, t.github_issue_id, t.github_pr_id, t.github_repo,
               t.deleted_at, t.created_at, t.updated_at
        FROM tasks t
        WHERE t.github_pr_id = $1
          AND t.github_repo = $2
          AND t.deleted_at IS NULL
        "#,
        pr_number,
        repo,
    )
    .fetch_all(db)
    .await?;

    for task in tasks {
        move_to_role_column_if_configured(db, hub, task, outcome).await?;
    }

    Ok(())
}

/// Which destination column a PR/issue close maps to.
#[derive(Clone, Copy)]
enum PrCloseOutcome {
    /// PR merged or issue closed → Done column.
    Merged,
    /// PR closed without merging → In Progress column.
    ClosedUnmerged,
}

/// Sets `github_repo` on tasks whose `short_id` appears in commit messages.
/// Partial auto-link: once the repo is recorded, PR/issue linking completes
/// the connection when a PR referencing those commits is opened.
#[tracing::instrument(skip(db))]
pub async fn autolink_from_push(
    db: &PgPool,
    repo: &str,
    short_ids: &[i64],
) -> Result<(), ApiError> {
    for &short_id in short_ids {
        let rows = sqlx::query!(
            "UPDATE tasks SET github_repo = $1, updated_at = NOW()
             WHERE short_id = $2 AND github_repo IS NULL AND deleted_at IS NULL
             RETURNING id",
            repo,
            short_id,
        )
        .fetch_all(db)
        .await?;

        if !rows.is_empty() {
            tracing::info!(short_id, repo, "Auto-linked task to repo via push");
        }
    }
    Ok(())
}

async fn move_to_role_column_if_configured(
    db: &PgPool,
    hub: &Arc<Hub>,
    task: Task,
    outcome: PrCloseOutcome,
) -> Result<(), ApiError> {
    let project = sqlx::query!(
        "SELECT done_column_id, in_progress_column_id, auto_close_on_issue_closed
         FROM projects WHERE id = $1 AND deleted_at IS NULL",
        task.project_id
    )
    .fetch_optional(db)
    .await?;

    let Some(project) = project else {
        return Ok(());
    };

    if !project.auto_close_on_issue_closed {
        return Ok(());
    }

    let (target_col, role) = match outcome {
        PrCloseOutcome::Merged => (project.done_column_id, "done"),
        PrCloseOutcome::ClosedUnmerged => (project.in_progress_column_id, "in-progress"),
    };

    let Some(done_col) = target_col else {
        tracing::debug!(
            project_id = %task.project_id,
            role,
            "auto-move is enabled but the target column is not configured"
        );
        return Ok(());
    };

    if task.column_id == done_col {
        return Ok(()); // already there
    }

    let max_pos: Option<f64> = sqlx::query_scalar!(
        "SELECT MAX(position) FROM tasks WHERE column_id = $1 AND deleted_at IS NULL",
        done_col
    )
    .fetch_one(db)
    .await?;
    let new_pos = max_pos.map(|p| p + 1.0).unwrap_or(1.0);

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
                  short_id, github_issue_id, github_pr_id, github_repo,
                  deleted_at, created_at, updated_at
        "#,
        done_col,
        new_pos,
        task.id,
    )
    .fetch_one(db)
    .await?;

    let labels = fetch_task_label_ids(db, updated.id).await?;
    let resp = TaskResponse::from(crate::services::task::helpers::TaskWithLabels {
        task: updated,
        labels,
    });

    publish_project_event(hub, resp.project_id, "task.moved", &resp);

    tracing::info!(
        task_id = %resp.id,
        column_id = %done_col,
        "Auto-moved task to Done column via webhook"
    );

    Ok(())
}
