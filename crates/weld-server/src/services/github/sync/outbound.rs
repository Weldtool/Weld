use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::realtime::Hub;
use crate::services::github::helpers::create_installation_token;
use crate::state::GithubConfig;

/// Called (fire-and-forget) after a task is moved to the project's Done
/// column.  If the task has a linked GitHub issue and the team has a GitHub
/// connection, posts a comment on that issue.
pub async fn maybe_post_done_comment(
    db: &PgPool,
    _hub: &Hub,
    github: &GithubConfig,
    task_id: Uuid,
    new_column_id: Uuid,
) {
    if let Err(e) = try_post_done_comment(db, github, task_id, new_column_id).await {
        tracing::warn!(task_id = %task_id, error = %e, "Failed to post GitHub done comment");
    }
}

async fn try_post_done_comment(
    db: &PgPool,
    github: &GithubConfig,
    task_id: Uuid,
    new_column_id: Uuid,
) -> Result<(), ApiError> {
    // Fetch task + project in one query
    let row = sqlx::query!(
        r#"
        SELECT t.github_issue_id, t.github_repo, t.title,
               p.team_id, p.done_column_id
        FROM tasks t
        JOIN projects p ON p.id = t.project_id
        WHERE t.id = $1 AND t.deleted_at IS NULL AND p.deleted_at IS NULL
        "#,
        task_id
    )
    .fetch_optional(db)
    .await?;

    let Some(row) = row else { return Ok(()) };

    // Only act if moved to the configured Done column
    if row.done_column_id != Some(new_column_id) {
        return Ok(());
    }

    let (Some(issue_number), Some(repo)) = (row.github_issue_id, row.github_repo) else {
        return Ok(());
    };

    let conn = sqlx::query!(
        "SELECT installation_id FROM github_connections WHERE team_id = $1",
        row.team_id
    )
    .fetch_optional(db)
    .await?;

    let Some(conn) = conn else { return Ok(()) };

    let token = create_installation_token(github, conn.installation_id).await?;

    let body = format!(
        "✅ Resolved via [Weld](https://weld.app): task **{}** was moved to Done.",
        row.title
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "https://api.github.com/repos/{repo}/issues/{issue_number}/comments"
        ))
        .bearer_auth(&token)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .json(&serde_json::json!({ "body": body }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub comment post failed ({status}): {text}"
        )));
    }

    tracing::info!(task_id = %task_id, repo, issue_number, "Posted Done comment on GitHub issue");
    Ok(())
}
