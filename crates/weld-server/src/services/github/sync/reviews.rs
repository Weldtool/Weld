//! Reviewing and merging pull requests from inside Weld.
//!
//! Weld could previously open a PR and react to one being merged, but nothing
//! in between: the review overlay had no diff to show and its verdict never
//! left the browser. These three calls close that loop.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

use crate::errors::ApiError;
use crate::services::github::helpers::{gh_client, github_error};
use crate::services::github::user_token::attribution_footer;

use super::list::resolve_repo_and_acting_token;
use super::list::resolve_repo_and_token;

/// What a reviewer decided. Maps onto GitHub's review `event` values.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Approve,
    Comment,
    RequestChanges,
}

impl ReviewVerdict {
    fn as_github_event(self) -> &'static str {
        match self {
            ReviewVerdict::Approve => "APPROVE",
            ReviewVerdict::Comment => "COMMENT",
            ReviewVerdict::RequestChanges => "REQUEST_CHANGES",
        }
    }
}

/// How to merge. Mirrors GitHub's `merge_method`.
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
    #[default]
    Merge,
    Squash,
    Rebase,
}

impl MergeMethod {
    fn as_str(self) -> &'static str {
        match self {
            MergeMethod::Merge => "merge",
            MergeMethod::Squash => "squash",
            MergeMethod::Rebase => "rebase",
        }
    }
}

/// Fetches a single pull request together with its changed files.
///
/// The PR list endpoint does not include file diffs, so the review UI had
/// nothing to render outside of study-mode fixtures. The two GitHub calls run
/// concurrently and the file list is merged in under a `files` key, matching
/// the shape the frontend already expects.
pub async fn get_project_pr(
    pool: &PgPool,
    github: &crate::state::GithubConfig,
    project_id: Uuid,
    number: i64,
    caller_id: Uuid,
) -> Result<serde_json::Value, ApiError> {
    let (repo, token) =
        resolve_repo_and_token(pool, github, project_id, caller_id, TeamRole::Viewer).await?;

    let pr_url = format!("https://api.github.com/repos/{repo}/pulls/{number}");
    // 300 is GitHub's hard ceiling for this endpoint; past that the diff is far
    // too large to review in a side panel anyway.
    let files_url = format!("{pr_url}/files?per_page=100");

    let (pr, files) = tokio::join!(
        fetch_json(&token, &pr_url, "pull request"),
        fetch_json(&token, &files_url, "pull request files"),
    );

    let mut pr = pr?;
    // A missing or unreadable file list should not blank out the whole review;
    // the metadata and verdict controls are still useful without the diff.
    let files = match files {
        Ok(files) => files,
        Err(e) => {
            tracing::warn!(repo, number, error = %e, "Could not load PR files");
            serde_json::Value::Array(Vec::new())
        }
    };

    if let Some(obj) = pr.as_object_mut() {
        obj.insert("files".into(), files);
    }

    Ok(pr)
}

/// Submits a review on a pull request. Requires Member+ on the project's team.
///
/// Posted with the reviewer's own GitHub token when they have linked their
/// account, so GitHub records *them* as the reviewer — which is the whole point
/// of a review history. Only when they have not linked does this fall back to
/// the App, and then a footer names them instead.
#[allow(clippy::too_many_arguments)] // a review genuinely needs all of these
pub async fn submit_pr_review(
    pool: &PgPool,
    github: &crate::state::GithubConfig,
    master_key: Option<&str>,
    project_id: Uuid,
    number: i64,
    caller_id: Uuid,
    verdict: ReviewVerdict,
    body: &str,
) -> Result<serde_json::Value, ApiError> {
    let (repo, acting) = resolve_repo_and_acting_token(
        pool,
        github,
        master_key,
        project_id,
        caller_id,
        TeamRole::Member,
    )
    .await?;

    let body = body.trim();
    // GitHub rejects an empty COMMENT or REQUEST_CHANGES review outright.
    if body.is_empty() && !matches!(verdict, ReviewVerdict::Approve) {
        return Err(ApiError::Validation(
            "A comment is required when commenting or requesting changes".into(),
        ));
    }

    // Acting as the user needs no footer: GitHub already shows their name on
    // the review. The footer exists only to rescue attribution when we had to
    // fall back to the app.
    let attributed = match attribution_footer(&acting, &display_name(pool, caller_id).await) {
        Some(footer) if body.is_empty() => footer,
        Some(footer) => format!("{body}\n\n---\n{footer}"),
        None => body.to_string(),
    };

    let payload = ReviewPayload {
        event: verdict.as_github_event(),
        body: &attributed,
    };

    let resp = gh_client()
        .post(format!(
            "https://api.github.com/repos/{repo}/pulls/{number}/reviews"
        ))
        .bearer_auth(&acting.token)
        .header("Accept", "application/vnd.github.v3+json")
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("submit review", resp).await);
    }

    tracing::info!(
        repo,
        number,
        verdict = ?verdict,
        as_user = acting.as_user,
        "Submitted PR review from Weld"
    );

    resp.json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse review response: {e}")))
}

/// Merges a pull request. Requires Member+ on the project's team.
///
/// Merged with the caller's own token when linked, so the merge commit and the
/// PR timeline name them. The resulting `pull_request.closed` webhook is what
/// advances the linked card, so this deliberately does not touch the board.
pub async fn merge_pr(
    pool: &PgPool,
    github: &crate::state::GithubConfig,
    master_key: Option<&str>,
    project_id: Uuid,
    number: i64,
    caller_id: Uuid,
    method: MergeMethod,
) -> Result<serde_json::Value, ApiError> {
    let (repo, acting) = resolve_repo_and_acting_token(
        pool,
        github,
        master_key,
        project_id,
        caller_id,
        TeamRole::Member,
    )
    .await?;

    let resp = gh_client()
        .put(format!(
            "https://api.github.com/repos/{repo}/pulls/{number}/merge"
        ))
        .bearer_auth(&acting.token)
        .header("Accept", "application/vnd.github.v3+json")
        .json(&MergePayload {
            merge_method: method.as_str(),
        })
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("merge pull request", resp).await);
    }

    tracing::info!(
        repo,
        number,
        method = method.as_str(),
        as_user = acting.as_user,
        "Merged PR from Weld"
    );

    resp.json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse merge response: {e}")))
}

async fn fetch_json(token: &str, url: &str, context: &str) -> Result<serde_json::Value, ApiError> {
    let resp = gh_client()
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error(context, resp).await);
    }

    resp.json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse {context}: {e}")))
}

#[derive(Serialize)]
struct ReviewPayload<'a> {
    event: &'a str,
    body: &'a str,
}

#[derive(Serialize)]
struct MergePayload<'a> {
    merge_method: &'a str,
}

/// Best-effort display name, used only for the fallback attribution footer.
async fn display_name(pool: &PgPool, user_id: Uuid) -> String {
    sqlx::query_scalar!("SELECT display_name FROM users WHERE id = $1", user_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "a Weld user".to_string())
}
