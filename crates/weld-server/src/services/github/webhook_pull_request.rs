use std::sync::Arc;

use sqlx::PgPool;

use crate::errors::ApiError;
use crate::realtime::Hub;

use super::{activity_feed, sync};

/// Handles `pull_request` and `pull_request_review` webhook events.
pub async fn handle(
    db: &PgPool,
    hub: &Arc<Hub>,
    event: &str,
    payload: &serde_json::Value,
) -> Result<(), ApiError> {
    let action = payload["action"].as_str().unwrap_or("");
    let repo = payload["repository"]["full_name"].as_str().unwrap_or("");
    let pr_number = payload["pull_request"]["number"].as_i64().unwrap_or(0);
    let merged = payload["pull_request"]["merged"].as_bool().unwrap_or(false);

    tracing::info!(event, action, repo, pr_number, "GitHub PR event");

    if event == "pull_request" {
        sync::on_pr_event(db, hub, repo, pr_number, action, merged).await?;
        activity_feed::post_pr_event(db, hub, repo, payload, action, merged).await?;
    } else if event == "pull_request_review" && action == "submitted" {
        activity_feed::post_review_event(db, hub, repo, payload).await?;
    }

    Ok(())
}
