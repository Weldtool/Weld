//! Weld → GitHub sync for work that happens on the board.
//!
//! Without this, the integration is one-way: GitHub could move a card, but
//! moving a card left no trace on GitHub. Anyone watching the issue saw silence
//! until the PR merged.
//!
//! Every function here is written to be spawned as a background task. GitHub
//! latency must never make a drag-and-drop feel slow, and a GitHub outage must
//! never fail a board update that already committed. They therefore log their
//! own failures and return `()`.

use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::services::github::helpers::{gh_client, github_error};
use crate::services::github::user_token::{ActingToken, acting_token_for_team};
use crate::state::GithubConfig;

/// Everything needed to talk to GitHub about one task.
struct TaskGithubContext {
    repo: String,
    issue_number: Option<i64>,
    acting: ActingToken,
}

/// Resolves a task's repo, target number and installation token.
/// Returns `None` when the task simply is not linked to GitHub.
async fn context_for_task(
    db: &PgPool,
    github: &GithubConfig,
    master_key: Option<&str>,
    task_id: Uuid,
    actor_id: Uuid,
) -> Result<Option<TaskGithubContext>, ApiError> {
    let row = sqlx::query!(
        r#"
        SELECT t.github_repo, t.github_issue_id, t.github_pr_id, p.team_id
        FROM tasks t
        JOIN projects p ON p.id = t.project_id AND p.deleted_at IS NULL
        JOIN github_connections c ON c.team_id = p.team_id
        WHERE t.id = $1 AND t.deleted_at IS NULL
        "#,
        task_id
    )
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let Some(repo) = row.github_repo else {
        return Ok(None);
    };

    let acting = acting_token_for_team(db, github, master_key, actor_id, row.team_id).await?;

    Ok(Some(TaskGithubContext {
        repo,
        issue_number: row.github_issue_id,
        acting,
    }))
}

/// Comments on a task's linked **issue** when it enters In Progress or Review.
///
/// Deliberately narrow. A merged PR already announces itself on GitHub, so a
/// "moved to Done" comment is redundant, and commenting on a PR at all is noise
/// when the PR shows its own state. What GitHub genuinely cannot tell you is
/// that a human picked the issue up — so that is the only thing we say.
///
/// Off per project via `projects.github_comment_on_move` for teams that want
/// the issue thread to stay human-only.
pub async fn comment_task_moved(
    db: PgPool,
    github: Arc<GithubConfig>,
    master_key: Option<String>,
    task_id: Uuid,
    actor_id: Uuid,
) {
    if let Err(e) =
        try_comment_task_moved(&db, &github, master_key.as_deref(), task_id, actor_id).await
    {
        tracing::warn!(task_id = %task_id, error = %e, "Could not comment task move to GitHub");
    }
}

async fn try_comment_task_moved(
    db: &PgPool,
    github: &GithubConfig,
    master_key: Option<&str>,
    task_id: Uuid,
    actor_id: Uuid,
) -> Result<(), ApiError> {
    let row = sqlx::query!(
        r#"
        SELECT t.column_id, col.name AS column_name,
               p.in_progress_column_id, p.review_column_id, p.github_comment_on_move
        FROM tasks t
        JOIN columns col ON col.id = t.column_id
        JOIN projects p ON p.id = t.project_id AND p.deleted_at IS NULL
        WHERE t.id = $1 AND t.deleted_at IS NULL
        "#,
        task_id
    )
    .fetch_optional(db)
    .await?;

    let Some(row) = row else { return Ok(()) };

    if !row.github_comment_on_move {
        return Ok(());
    }

    // Done is deliberately absent: the merge that got it there already said so.
    let announces_pickup =
        [row.in_progress_column_id, row.review_column_id].contains(&Some(row.column_id));

    if !announces_pickup {
        return Ok(());
    }

    let Some(ctx) = context_for_task(db, github, master_key, task_id, actor_id).await? else {
        return Ok(());
    };

    // Only issues get this. A PR narrates its own state perfectly well.
    let Some(issue_number) = ctx.issue_number else {
        return Ok(());
    };

    let actor = display_name(db, actor_id).await;
    let column = row.column_name;
    let body = format!("**{actor}** moved this to **{column}** in Weld.");
    post_comment(&ctx, issue_number, &body).await
}

/// Mirrors a task's edited title/description onto its linked GitHub issue.
///
/// Only touches issues: a PR's body belongs to whoever opened it.
pub async fn sync_task_to_issue(
    db: PgPool,
    github: Arc<GithubConfig>,
    master_key: Option<String>,
    task_id: Uuid,
    actor_id: Uuid,
    title: String,
    description: Option<String>,
) {
    if let Err(e) = try_sync_task_to_issue(
        &db,
        &github,
        master_key.as_deref(),
        task_id,
        actor_id,
        &title,
        description.as_deref(),
    )
    .await
    {
        tracing::warn!(task_id = %task_id, error = %e, "Could not sync task fields to GitHub");
    }
}

async fn try_sync_task_to_issue(
    db: &PgPool,
    github: &GithubConfig,
    master_key: Option<&str>,
    task_id: Uuid,
    actor_id: Uuid,
    title: &str,
    description: Option<&str>,
) -> Result<(), ApiError> {
    let Some(ctx) = context_for_task(db, github, master_key, task_id, actor_id).await? else {
        return Ok(());
    };
    let Some(issue_number) = ctx.issue_number else {
        return Ok(());
    };

    let repo = &ctx.repo;
    let resp = gh_client()
        .patch(format!(
            "https://api.github.com/repos/{repo}/issues/{issue_number}"
        ))
        .bearer_auth(&ctx.acting.token)
        .header("Accept", "application/vnd.github.v3+json")
        .json(&serde_json::json!({
            "title": title,
            "body": description.unwrap_or(""),
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("update issue", resp).await);
    }

    tracing::info!(task_id = %task_id, repo, issue_number, "Synced task fields to GitHub issue");
    Ok(())
}

/// Best-effort display name for comment attribution.
async fn display_name(db: &PgPool, user_id: Uuid) -> String {
    sqlx::query_scalar!("SELECT display_name FROM users WHERE id = $1", user_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Someone".to_string())
}

async fn post_comment(ctx: &TaskGithubContext, number: i64, body: &str) -> Result<(), ApiError> {
    let repo = &ctx.repo;

    let resp = gh_client()
        .post(format!(
            "https://api.github.com/repos/{repo}/issues/{number}/comments"
        ))
        .bearer_auth(&ctx.acting.token)
        .header("Accept", "application/vnd.github.v3+json")
        .json(&serde_json::json!({ "body": body }))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("post comment", resp).await);
    }

    tracing::info!(repo, number, "Posted Weld activity comment to GitHub");
    Ok(())
}
