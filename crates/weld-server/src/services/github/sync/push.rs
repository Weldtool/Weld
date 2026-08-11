use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::{Priority, TeamRole};
use weld_core::models::task::Task;

use crate::errors::ApiError;
use crate::services::github::helpers::{gh_client, github_error};
use crate::services::github::user_token::acting_token_for_team;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use crate::services::task::helpers::{TaskWithLabels, fetch_active_task, fetch_task_label_ids};
use crate::state::GithubConfig;

/// Creates a new GitHub issue mirroring a Weld task and stores the
/// returned issue number on the task. Requires Member+ on the project's
/// team. Errors if the task is already linked or the project has no
/// `github_repo` configured.
pub async fn push_task_to_github(
    pool: &PgPool,
    github: &GithubConfig,
    master_key: Option<&str>,
    task_id: Uuid,
    caller_id: Uuid,
    title_override: Option<String>,
    body_override: Option<String>,
) -> Result<TaskWithLabels, ApiError> {
    let task = fetch_active_task(pool, task_id).await?;
    let project = fetch_active_project(pool, task.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    if task.github_issue_id.is_some() {
        return Err(ApiError::Conflict(
            "Task is already linked to a GitHub issue".into(),
        ));
    }

    let repo = project.github_repo.clone().ok_or_else(|| {
        ApiError::Validation("Project has no GitHub repository configured".into())
    })?;

    let acting =
        acting_token_for_team(pool, github, master_key, caller_id, project.team_id).await?;

    // Pull the names of any Weld labels attached to the task — GitHub
    // matches labels by name and will create missing labels on its end.
    let label_names: Vec<String> = sqlx::query_scalar!(
        r#"
        SELECT l.name
        FROM task_labels tl
        JOIN labels l ON l.id = tl.label_id
        WHERE tl.task_id = $1
        "#,
        task_id,
    )
    .fetch_all(pool)
    .await?;

    let title = title_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&task.title);
    let body = body_override
        .as_deref()
        .or(task.description.as_deref())
        .unwrap_or("");

    let payload = CreateIssuePayload {
        title,
        body,
        labels: label_names.clone(),
    };

    let resp = gh_client()
        .post(format!("https://api.github.com/repos/{repo}/issues"))
        .bearer_auth(&acting.token)
        .header("Accept", "application/vnd.github.v3+json")
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("create issue", resp).await);
    }

    let created: CreatedIssue = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub response: {e}")))?;

    let updated = sqlx::query_as!(
        Task,
        r#"
        UPDATE tasks
        SET github_repo = $2, github_issue_id = $3, updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING id, project_id, column_id, title, description,
                  assignee_id, reporter_id,
                  priority as "priority: Priority",
                  position, due_date, estimate_hours,
                  short_id, github_issue_id, github_pr_id, github_repo, github_check_status,
                  deleted_at, created_at, updated_at
        "#,
        task_id,
        repo,
        created.number,
    )
    .fetch_one(pool)
    .await?;

    let labels = fetch_task_label_ids(pool, task_id).await?;
    Ok(TaskWithLabels {
        task: updated,
        labels,
    })
}

#[derive(Serialize)]
struct CreateIssuePayload<'a> {
    title: &'a str,
    body: &'a str,
    labels: Vec<String>,
}

#[derive(Deserialize)]
struct CreatedIssue {
    number: i64,
}
