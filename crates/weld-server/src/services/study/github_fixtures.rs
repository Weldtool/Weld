//! Static GitHub fixtures served from the embedded seed under STUDY_MODE.
//!
//! The product's GitHub view proxies the live GitHub API; for the study we must
//! render identical, deterministic content with no network calls and no writes
//! (per `research-paper/CLAUDE_STUDY.md` §4). These functions reshape the seed's
//! `github` section into the exact GitHub-API JSON the frontend already
//! consumes, so the view renders unchanged. The content is the same for every
//! participant, so no per-participant storage is needed.

use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

use super::seed::SEED_JSON;
use crate::errors::ApiError;
use crate::services::github::sync::{IssueTemplate, RepoBranches};
use crate::services::project::helpers::fetch_active_project;
use crate::services::team::helpers::require_team_role;

#[derive(Debug, Deserialize)]
struct SeedRoot {
    github: GithubFixtures,
}

#[derive(Debug, Deserialize)]
struct GithubFixtures {
    repo: FxRepo,
    issues: Vec<FxIssue>,
    pull_requests: Vec<FxPr>,
}

#[derive(Debug, Deserialize)]
struct FxRepo {
    full_name: String,
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct FxIssue {
    number: i64,
    title: String,
    state: String,
    author: String,
    body: String,
    #[serde(default)]
    labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FxPr {
    number: i64,
    title: String,
    state: String,
    author: String,
    body: String,
    #[serde(default)]
    labels: Vec<String>,
}

fn fixtures() -> Result<GithubFixtures, ApiError> {
    serde_json::from_str::<SeedRoot>(SEED_JSON)
        .map(|r| r.github)
        .map_err(|e| ApiError::Internal(format!("Invalid study GitHub fixtures: {e}")))
}

/// Verify the caller may view the project (Viewer+), matching the live path's
/// access control. The fixture content itself is identical for all participants.
async fn check_access(pool: &PgPool, project_id: Uuid, caller_id: Uuid) -> Result<(), ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_team_role(pool, project.team_id, caller_id, TeamRole::Viewer).await?;
    Ok(())
}

/// Map a fixture symbolic author (`user_sarah`) to a plausible GitHub login.
fn login_for(author: &str) -> &'static str {
    match author {
        "user_sarah" => "sarahchen",
        "user_alex" => "alexrivera",
        "user_priya" => "priyanair",
        "user_marcus" => "marcuswebb",
        _ => "pettracker-bot",
    }
}

/// GitHub label objects carry a color; fixtures store only names, so assign a
/// stable color per known label name (neutral gray otherwise).
fn label_objects(names: &[String]) -> Vec<Value> {
    names
        .iter()
        .map(|name| {
            let color = match name.as_str() {
                "bug" => "d73a4a",
                "feature" => "0e8a16",
                "frontend" => "5319e7",
                "backend" => "fbca04",
                _ => "ededed",
            };
            json!({ "name": name, "color": color })
        })
        .collect()
}

fn issue_value(repo: &str, issue: &FxIssue, include_body: bool) -> Value {
    let mut v = json!({
        "number": issue.number,
        "title": issue.title,
        "state": issue.state,
        "html_url": format!("https://github.com/{repo}/issues/{}", issue.number),
        "user": { "login": login_for(&issue.author), "avatar_url": "" },
        "labels": label_objects(&issue.labels),
    });
    if include_body {
        v["body"] = json!(issue.body);
    }
    v
}

fn pr_value(repo: &str, pr: &FxPr) -> Value {
    json!({
        "number": pr.number,
        "title": pr.title,
        "state": pr.state,
        "draft": false,
        "html_url": format!("https://github.com/{repo}/pull/{}", pr.number),
        "body": pr.body,
        "user": { "login": login_for(&pr.author), "avatar_url": "" },
        "labels": label_objects(&pr.labels),
    })
}

pub async fn list_issues(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<Value>, ApiError> {
    check_access(pool, project_id, caller_id).await?;
    let fx = fixtures()?;
    Ok(fx
        .issues
        .iter()
        .map(|i| issue_value(&fx.repo.full_name, i, false))
        .collect())
}

pub async fn get_issue(
    pool: &PgPool,
    project_id: Uuid,
    issue_number: i64,
    caller_id: Uuid,
) -> Result<Value, ApiError> {
    check_access(pool, project_id, caller_id).await?;
    let fx = fixtures()?;
    fx.issues
        .iter()
        .find(|i| i.number == issue_number)
        .map(|i| issue_value(&fx.repo.full_name, i, true))
        .ok_or_else(|| ApiError::NotFound(format!("Issue #{issue_number} not found")))
}

pub async fn list_prs(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<Value>, ApiError> {
    check_access(pool, project_id, caller_id).await?;
    let fx = fixtures()?;
    Ok(fx
        .pull_requests
        .iter()
        .map(|p| pr_value(&fx.repo.full_name, p))
        .collect())
}

pub async fn list_branches(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<RepoBranches, ApiError> {
    check_access(pool, project_id, caller_id).await?;
    let fx = fixtures()?;
    Ok(RepoBranches {
        default_branch: fx.repo.default_branch.clone(),
        branches: vec![fx.repo.default_branch],
    })
}

/// No PR template in the study fixtures.
pub async fn get_pr_template(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Option<String>, ApiError> {
    check_access(pool, project_id, caller_id).await?;
    Ok(None)
}

/// No issue templates in the study fixtures.
pub async fn list_issue_templates(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<IssueTemplate>, ApiError> {
    check_access(pool, project_id, caller_id).await?;
    Ok(Vec::new())
}
