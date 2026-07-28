use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::Priority;
use weld_core::models::task::Task;

use crate::errors::ApiError;
use crate::realtime::Hub;
use crate::realtime::broadcast::publish_project_event;
use crate::routes::tasks::TaskResponse;
use crate::services::task::helpers::fetch_task_label_ids;

/// Loads the non-deleted tasks linked to a GitHub issue or PR in `repo`,
/// restricted to projects owned by `teams`.
///
/// The team filter is the tenant boundary: `github_repo` is a free-text string,
/// so matching on it alone would let one installation's webhooks reach another
/// customer's board.
async fn linked_tasks(
    db: &PgPool,
    repo: &str,
    teams: &[Uuid],
    issue_number: Option<i64>,
    pr_number: Option<i64>,
) -> Result<Vec<Task>, ApiError> {
    let tasks = sqlx::query_as!(
        Task,
        r#"
        SELECT t.id, t.project_id, t.column_id, t.title, t.description,
               t.assignee_id, t.reporter_id,
               t.priority as "priority: Priority",
               t.position, t.due_date, t.estimate_hours,
               t.short_id, t.github_issue_id, t.github_pr_id, t.github_repo, t.github_check_status,
               t.deleted_at, t.created_at, t.updated_at
        FROM tasks t
        JOIN projects p ON p.id = t.project_id AND p.deleted_at IS NULL
        WHERE t.github_repo = $1
          AND t.deleted_at IS NULL
          AND p.team_id = ANY($2)
          AND (($3::bigint IS NOT NULL AND t.github_issue_id = $3)
            OR ($4::bigint IS NOT NULL AND t.github_pr_id = $4))
        "#,
        repo,
        teams,
        issue_number,
        pr_number,
    )
    .fetch_all(db)
    .await?;

    Ok(tasks)
}

/// Called by the `issues` webhook handler.
///
/// - closed  → linked tasks move to the project's Done column
/// - reopened → back to In Progress
/// - edited  → title/description are copied onto the linked tasks
///
/// Column moves are gated on the project's auto-move setting.
#[tracing::instrument(skip(db, hub, payload))]
pub async fn on_issue_event(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    issue_number: i64,
    action: &str,
    payload: &serde_json::Value,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    let outcome = match action {
        "closed" => Some(PrSyncOutcome::Merged),
        "reopened" => Some(PrSyncOutcome::ClosedUnmerged),
        "edited" => None,
        _ => return Ok(()),
    };

    let tasks = linked_tasks(db, repo, teams, Some(issue_number), None).await?;

    for task in tasks {
        match outcome {
            Some(outcome) => move_to_role_column_if_configured(db, hub, task, outcome).await?,
            None => apply_issue_edit(db, hub, task, payload).await?,
        }
    }

    Ok(())
}

/// Copies an edited issue's title and body onto the linked task.
///
/// Writes straight to the row rather than going through `update_task`: the edit
/// originates on GitHub, so re-entering the outbound sync path would bounce the
/// same text back and loop.
async fn apply_issue_edit(
    db: &PgPool,
    hub: &Arc<Hub>,
    task: Task,
    payload: &serde_json::Value,
) -> Result<(), ApiError> {
    let issue = &payload["issue"];
    let title = issue["title"].as_str().unwrap_or("").trim().to_string();
    // GitHub sends `body: null` for an emptied description.
    let body = issue["body"].as_str().map(str::to_string);

    if title.is_empty() {
        return Ok(());
    }
    if title == task.title && body == task.description {
        return Ok(());
    }

    let updated = sqlx::query_as!(
        Task,
        r#"
        UPDATE tasks
        SET title = $2, description = $3, updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING id, project_id, column_id, title, description,
                  assignee_id, reporter_id,
                  priority as "priority: Priority",
                  position, due_date, estimate_hours,
                  short_id, github_issue_id, github_pr_id, github_repo, github_check_status,
                  deleted_at, created_at, updated_at
        "#,
        task.id,
        title,
        body,
    )
    .fetch_one(db)
    .await?;

    let labels = fetch_task_label_ids(db, updated.id).await?;
    let resp = TaskResponse::from(crate::services::task::helpers::TaskWithLabels {
        task: updated,
        labels,
    });
    publish_project_event(hub, resp.project_id, "task.updated", &resp);

    tracing::info!(task_id = %resp.id, "Synced edited GitHub issue onto task");
    Ok(())
}

/// Called by the `pull_request` webhook handler. Moves the linked task as the
/// PR changes state: a merged PR sends it to the Done column, a PR closed
/// without merging sends it back to In Progress, and a reopened PR sends it
/// back to Review. All are gated on the project's auto-move setting.
#[tracing::instrument(skip(db, hub))]
pub async fn on_pr_event(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    pr_number: i64,
    action: &str,
    merged: bool,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    let outcome = match action {
        "closed" if merged => PrSyncOutcome::Merged,
        "closed" => PrSyncOutcome::ClosedUnmerged,
        "reopened" => PrSyncOutcome::Reopened,
        // A draft marked ready is the moment it actually wants review.
        "ready_for_review" => PrSyncOutcome::Reopened,
        _ => return Ok(()),
    };

    let tasks = linked_tasks(db, repo, teams, None, Some(pr_number)).await?;

    for task in tasks {
        move_to_role_column_if_configured(db, hub, task, outcome).await?;
    }

    Ok(())
}

/// Which destination column a PR/issue state change maps to.
#[derive(Clone, Copy)]
enum PrSyncOutcome {
    /// PR merged or issue closed → Done column.
    Merged,
    /// PR closed without merging → In Progress column.
    ClosedUnmerged,
    /// Closed PR reopened → Review column.
    Reopened,
}

/// Sets `github_repo` on tasks whose `short_id` appears in commit messages.
/// Partial auto-link: once the repo is recorded, PR/issue linking completes
/// the connection when a PR referencing those commits is opened.
#[tracing::instrument(skip(db))]
pub async fn autolink_from_push(
    db: &PgPool,
    repo: &str,
    short_ids: &[i64],
    teams: &[Uuid],
) -> Result<(), ApiError> {
    for &short_id in short_ids {
        let rows = sqlx::query!(
            "UPDATE tasks t SET github_repo = $1, updated_at = NOW()
             FROM projects p
             WHERE p.id = t.project_id
               AND p.deleted_at IS NULL
               AND p.team_id = ANY($3)
               AND t.short_id = $2
               AND t.github_repo IS NULL
               AND t.deleted_at IS NULL
             RETURNING t.id",
            repo,
            short_id,
            teams,
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
    outcome: PrSyncOutcome,
) -> Result<(), ApiError> {
    let project = sqlx::query!(
        "SELECT done_column_id, in_progress_column_id, review_column_id, auto_close_on_issue_closed
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
        PrSyncOutcome::Merged => (project.done_column_id, "done"),
        PrSyncOutcome::ClosedUnmerged => (project.in_progress_column_id, "in-progress"),
        PrSyncOutcome::Reopened => (project.review_column_id, "review"),
    };

    let Some(target_col) = target_col else {
        tracing::debug!(
            project_id = %task.project_id,
            role,
            "auto-move is enabled but the target column is not configured"
        );
        return Ok(());
    };

    if task.column_id == target_col {
        return Ok(()); // already there
    }

    let max_pos: Option<f64> = sqlx::query_scalar!(
        "SELECT MAX(position) FROM tasks WHERE column_id = $1 AND deleted_at IS NULL",
        target_col
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
                  short_id, github_issue_id, github_pr_id, github_repo, github_check_status,
                  deleted_at, created_at, updated_at
        "#,
        target_col,
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
        column_id = %target_col,
        role,
        "Auto-moved task to role column via webhook"
    );

    Ok(())
}

/// Records a CI conclusion on every task linked to one of `pr_numbers`.
///
/// Scoped to `teams` for the same reason every other webhook path is: repo names
/// are not tenant boundaries.
#[tracing::instrument(skip(db, hub))]
pub async fn set_check_status(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    pr_numbers: &[i64],
    status: &str,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    let updated = sqlx::query_as!(
        Task,
        r#"
        UPDATE tasks t
        SET github_check_status = $4, updated_at = NOW()
        FROM projects p
        WHERE p.id = t.project_id
          AND p.deleted_at IS NULL
          AND p.team_id = ANY($3)
          AND t.github_repo = $1
          AND t.github_pr_id = ANY($2)
          AND t.deleted_at IS NULL
          AND t.github_check_status IS DISTINCT FROM $4
        RETURNING t.id, t.project_id, t.column_id, t.title, t.description,
                  t.assignee_id, t.reporter_id,
                  t.priority as "priority: Priority",
                  t.position, t.due_date, t.estimate_hours,
                  t.short_id, t.github_issue_id, t.github_pr_id, t.github_repo,
                  t.github_check_status,
                  t.deleted_at, t.created_at, t.updated_at
        "#,
        repo,
        pr_numbers,
        teams,
        status,
    )
    .fetch_all(db)
    .await?;

    for task in updated {
        let labels = fetch_task_label_ids(db, task.id).await?;
        let resp =
            TaskResponse::from(crate::services::task::helpers::TaskWithLabels { task, labels });
        publish_project_event(hub, resp.project_id, "task.updated", &resp);
    }

    Ok(())
}
