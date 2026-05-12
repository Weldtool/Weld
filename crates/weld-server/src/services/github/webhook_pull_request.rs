use sqlx::PgPool;

use crate::errors::ApiError;

/// Handles `pull_request` and `pull_request_review` webhook events.
///
/// Task ↔ PR linking is implemented in issue #29.  For now the handler
/// records the event at INFO level so deliveries are properly acknowledged.
pub async fn handle(
    _db: &PgPool,
    event: &str,
    payload: &serde_json::Value,
) -> Result<(), ApiError> {
    let action = payload["action"].as_str().unwrap_or("");
    let repo = payload["repository"]["full_name"].as_str().unwrap_or("");
    let pr_number = payload["pull_request"]["number"]
        .as_i64()
        .unwrap_or_else(|| payload["review"]["pull_request_url"].as_i64().unwrap_or(0));

    tracing::info!(
        event,
        action,
        repo,
        pr_number,
        "GitHub PR event received (task linking pending #29)"
    );

    Ok(())
}
