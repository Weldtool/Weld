use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::services::github::helpers::create_installation_token;
use crate::services::project::helpers::fetch_active_project;
use crate::services::team::helpers::require_team_role;
use crate::state::GithubConfig;
use weld_core::models::enums::TeamRole;

/// Proxies GitHub's issue list for the project's connected repository.
/// Returns raw JSON values so callers can decide which fields to expose.
pub async fn list_project_issues(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<serde_json::Value>, ApiError> {
    let (repo, token) = resolve_repo_and_token(pool, github, project_id, caller_id).await?;
    fetch_github_list(
        &token,
        &format!("https://api.github.com/repos/{repo}/issues?state=open&per_page=50"),
    )
    .await
}

/// Proxies GitHub's pull-request list for the project's connected repository.
pub async fn list_project_prs(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<serde_json::Value>, ApiError> {
    let (repo, token) = resolve_repo_and_token(pool, github, project_id, caller_id).await?;
    fetch_github_list(
        &token,
        &format!("https://api.github.com/repos/{repo}/pulls?state=open&per_page=50"),
    )
    .await
}

async fn resolve_repo_and_token(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<(String, String), ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_team_role(pool, project.team_id, caller_id, TeamRole::Viewer).await?;

    let repo = project.github_repo.ok_or_else(|| {
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
    Ok((repo, token))
}

async fn fetch_github_list(token: &str, url: &str) -> Result<Vec<serde_json::Value>, ApiError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub list request failed ({status}): {body}"
        )));
    }

    resp.json::<Vec<serde_json::Value>>()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub response: {e}")))
}
