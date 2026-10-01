use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::services::github::helpers::{create_installation_token, gh_client, github_error};
use crate::services::github::user_token::{ActingToken, acting_token_for_team};
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use crate::state::GithubConfig;
use weld_core::models::enums::TeamRole;

/// Proxies GitHub's issue list for the project's connected repository.
/// Returns raw JSON values so callers can decide which fields to expose.
///
/// `state` accepts GitHub's `open` / `closed` / `all`; anything else falls back
/// to `open`.
pub async fn list_project_issues(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
    state: Option<&str>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    let (repo, token) =
        resolve_repo_and_token(pool, github, project_id, caller_id, TeamRole::Viewer).await?;
    let state = normalize_state(state);
    let issues = fetch_github_list(
        &token,
        &format!("https://api.github.com/repos/{repo}/issues?state={state}&per_page=100"),
    )
    .await?;

    // GitHub's issues endpoint returns pull requests too — they carry a
    // `pull_request` key. Without this filter every PR showed up in the
    // "link an issue" picker.
    Ok(issues
        .into_iter()
        .filter(|i| i.get("pull_request").is_none())
        .collect())
}

/// Clamps a caller-supplied state filter to the values GitHub accepts.
fn normalize_state(state: Option<&str>) -> &'static str {
    match state {
        Some("closed") => "closed",
        Some("all") => "all",
        _ => "open",
    }
}

/// Proxies a single GitHub issue (by number) for the project's connected repository.
/// Returns the raw JSON object — callers decide which fields to expose.
pub async fn get_project_issue(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    issue_number: i64,
    caller_id: Uuid,
) -> Result<serde_json::Value, ApiError> {
    let (repo, token) =
        resolve_repo_and_token(pool, github, project_id, caller_id, TeamRole::Viewer).await?;
    fetch_github_object(
        &token,
        &format!("https://api.github.com/repos/{repo}/issues/{issue_number}"),
    )
    .await
}

/// Proxies GitHub's pull-request list for the project's connected repository.
pub async fn list_project_prs(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
    state: Option<&str>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    let (repo, token) =
        resolve_repo_and_token(pool, github, project_id, caller_id, TeamRole::Viewer).await?;
    let state = normalize_state(state);
    fetch_github_list(
        &token,
        &format!("https://api.github.com/repos/{repo}/pulls?state={state}&per_page=100"),
    )
    .await
}

/// Resolves a project's connected repository (`owner/repo`) and an installation
/// access token, after checking the caller holds at least `role`.
///
/// Reads use this. Writes want [`resolve_repo_and_acting_token`] instead, so the
/// action is attributed to the person who took it.
pub(crate) async fn resolve_repo_and_token(
    pool: &PgPool,
    github: &GithubConfig,
    project_id: Uuid,
    caller_id: Uuid,
    role: TeamRole,
) -> Result<(String, String), ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    // Project-level access, not just team membership: Members and Viewers only
    // reach the GitHub data of projects they have been added to.
    require_project_team_role(pool, &project, caller_id, role).await?;

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

/// Like [`resolve_repo_and_token`], but returns a token that acts as `caller_id`
/// when they have linked their GitHub account.
///
/// Every write goes through here so GitHub's own history answers "who did this?"
/// rather than naming the Weld bot for everything.
pub(crate) async fn resolve_repo_and_acting_token(
    pool: &PgPool,
    github: &GithubConfig,
    master_key: Option<&str>,
    project_id: Uuid,
    caller_id: Uuid,
    role: TeamRole,
) -> Result<(String, ActingToken), ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    // Project-level access, not just team membership: Members and Viewers only
    // reach the GitHub data of projects they have been added to.
    require_project_team_role(pool, &project, caller_id, role).await?;

    let repo = project.github_repo.ok_or_else(|| {
        ApiError::Validation("Project has no GitHub repository configured".into())
    })?;

    let acting =
        acting_token_for_team(pool, github, master_key, caller_id, project.team_id).await?;
    Ok((repo, acting))
}

async fn fetch_github_list(token: &str, url: &str) -> Result<Vec<serde_json::Value>, ApiError> {
    let resp = gh_client()
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("list request", resp).await);
    }

    resp.json::<Vec<serde_json::Value>>()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub response: {e}")))
}

async fn fetch_github_object(token: &str, url: &str) -> Result<serde_json::Value, ApiError> {
    let resp = gh_client()
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("request", resp).await);
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub response: {e}")))
}
