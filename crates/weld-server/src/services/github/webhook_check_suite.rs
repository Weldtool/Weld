use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::realtime::Hub;

use super::sync;

/// Handles `check_suite` webhook events.
///
/// Whether CI is green is the first thing anyone wants to know about a PR, and
/// the board had no way to show it. This records the suite's conclusion on any
/// task linked to the suite's pull requests.
///
/// Only `check_suite` is handled, not `check_run`: a suite is the roll-up of its
/// runs, so tracking runs individually would mean re-deriving the roll-up here.
pub async fn handle(
    db: &PgPool,
    hub: &Arc<Hub>,
    payload: &serde_json::Value,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    let action = payload["action"].as_str().unwrap_or("");
    let suite = &payload["check_suite"];

    let status = match action {
        // GitHub only sets `conclusion` once the suite finishes.
        "completed" => suite["conclusion"].as_str().unwrap_or("neutral"),
        "requested" | "rerequested" => "pending",
        _ => return Ok(()),
    };

    let repo = payload["repository"]["full_name"].as_str().unwrap_or("");

    // A suite can cover several PRs (shared head commit); an empty list means
    // the suite ran on a branch with no open PR, which no card tracks.
    let pr_numbers: Vec<i64> = suite["pull_requests"]
        .as_array()
        .map(|prs| prs.iter().filter_map(|pr| pr["number"].as_i64()).collect())
        .unwrap_or_default();

    if pr_numbers.is_empty() {
        return Ok(());
    }

    tracing::info!(
        repo,
        action,
        status,
        ?pr_numbers,
        "GitHub check_suite event"
    );

    sync::set_check_status(db, hub, repo, &pr_numbers, status, teams).await
}
