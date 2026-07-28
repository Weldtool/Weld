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

/// Caller-supplied fields for opening a pull request from a task.
pub struct CreatePrInput {
    /// Source branch containing the changes.
    pub head: String,
    /// Target branch to merge into.
    pub base: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
}

/// Opens a GitHub pull request for a task and stores the returned PR number on
/// the task (`github_pr_id`). If the task already has a linked issue, a
/// `Closes #N` line is appended to the body so merging the PR closes the issue.
/// Requires Member+ on the project's team.
///
/// Opened as the caller when they have linked GitHub, so the PR has a real
/// author rather than every PR in the repo appearing to come from the bot.
pub async fn create_pr_for_task(
    pool: &PgPool,
    github: &GithubConfig,
    master_key: Option<&str>,
    task_id: Uuid,
    caller_id: Uuid,
    input: CreatePrInput,
) -> Result<TaskWithLabels, ApiError> {
    let task = fetch_active_task(pool, task_id).await?;
    let project = fetch_active_project(pool, task.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    if task.github_pr_id.is_some() {
        return Err(ApiError::Conflict(
            "Task is already linked to a GitHub pull request".into(),
        ));
    }

    if input.head.trim().is_empty() || input.base.trim().is_empty() {
        return Err(ApiError::Validation(
            "Both head and base branches are required".into(),
        ));
    }
    if input.title.trim().is_empty() {
        return Err(ApiError::Validation("PR title is required".into()));
    }

    let repo = project.github_repo.clone().ok_or_else(|| {
        ApiError::Validation("Project has no GitHub repository configured".into())
    })?;

    let acting =
        acting_token_for_team(pool, github, master_key, caller_id, project.team_id).await?;

    // Link the PR back to the task's issue so a merge auto-closes it.
    let mut body = input.body;
    if let Some(issue_number) = task.github_issue_id {
        let reference = format!("Closes #{issue_number}");
        if !body.contains(&reference) {
            if !body.is_empty() {
                body.push_str("\n\n");
            }
            body.push_str(&reference);
        }
    }

    let payload = CreatePrPayload {
        title: input.title.trim(),
        head: input.head.trim(),
        base: input.base.trim(),
        body: &body,
        draft: input.draft,
    };

    let resp = gh_client()
        .post(format!("https://api.github.com/repos/{repo}/pulls"))
        .bearer_auth(&acting.token)
        .header("Accept", "application/vnd.github.v3+json")
        .json(&payload)
        .send()
        .await?;

    // 422 here means "no commits between base and head", a missing branch, or
    // an existing PR — the shared mapper turns that into a 400 with GitHub's
    // own message rather than a 500.
    if !resp.status().is_success() {
        return Err(github_error("create pull request", resp).await);
    }

    let created: CreatedPr = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub response: {e}")))?;

    let updated = sqlx::query_as!(
        Task,
        r#"
        UPDATE tasks
        SET github_repo = $2, github_pr_id = $3, updated_at = NOW()
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
struct CreatePrPayload<'a> {
    title: &'a str,
    head: &'a str,
    base: &'a str,
    body: &'a str,
    draft: bool,
}

#[derive(Deserialize)]
struct CreatedPr {
    number: i64,
}
