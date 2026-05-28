use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::{Priority, TeamRole};
use weld_core::models::task::Task;

use crate::errors::ApiError;
use crate::services::github::helpers::create_installation_token;
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
pub async fn create_pr_for_task(
    pool: &PgPool,
    github: &GithubConfig,
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

    let conn = sqlx::query!(
        "SELECT installation_id FROM github_connections WHERE team_id = $1",
        project.team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::Validation("Team has no GitHub connection".into()))?;

    let token = create_installation_token(github, conn.installation_id).await?;

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

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("https://api.github.com/repos/{repo}/pulls"))
        .bearer_auth(&token)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        // 422 is GitHub's catch-all for "no commits between base and head",
        // a missing branch, or an existing PR — surface it as a user error.
        if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
            let message = parse_github_error(&text)
                .unwrap_or_else(|| "branch has no commits to merge or already has a PR".into());
            return Err(ApiError::Validation(format!(
                "GitHub rejected the pull request: {message}"
            )));
        }
        return Err(ApiError::Internal(format!(
            "GitHub create-PR request failed ({status}): {text}"
        )));
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
                  short_id, github_issue_id, github_pr_id, github_repo,
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

/// Pulls the human-readable message out of a GitHub error response body.
fn parse_github_error(text: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    if let Some(errors) = json["errors"].as_array() {
        let joined: Vec<String> = errors
            .iter()
            .filter_map(|e| e["message"].as_str().map(str::to_string))
            .collect();
        if !joined.is_empty() {
            return Some(joined.join("; "));
        }
    }
    json["message"].as_str().map(str::to_string)
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
