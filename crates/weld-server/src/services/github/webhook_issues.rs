use std::sync::Arc;

use sqlx::PgPool;

use uuid::Uuid;

use crate::errors::ApiError;
use crate::realtime::Hub;

use super::{activity_feed, sync};

/// Handles `issues` and `issue_comment` webhook events.
pub async fn handle(
    db: &PgPool,
    hub: &Arc<Hub>,
    event: &str,
    payload: &serde_json::Value,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    let action = payload["action"].as_str().unwrap_or("");
    let repo = payload["repository"]["full_name"].as_str().unwrap_or("");
    let issue_number = payload["issue"]["number"].as_i64().unwrap_or(0);

    tracing::info!(event, action, repo, issue_number, "GitHub issue event");

    if event == "issues" {
        sync::on_issue_event(db, hub, repo, issue_number, action, payload, teams).await?;
        activity_feed::post_issue_event(db, hub, repo, payload, action, teams).await?;
    }

    Ok(())
}
