use sqlx::PgPool;

use crate::errors::ApiError;

/// Handles `issues` and `issue_comment` webhook events.
///
/// Task ↔ issue linking is implemented in issue #29.  For now the handler
/// records the event at INFO level so deliveries are properly acknowledged.
pub async fn handle(
    _db: &PgPool,
    event: &str,
    payload: &serde_json::Value,
) -> Result<(), ApiError> {
    let action = payload["action"].as_str().unwrap_or("");
    let repo = payload["repository"]["full_name"].as_str().unwrap_or("");
    let issue_number = payload["issue"]["number"].as_i64().unwrap_or(0);

    tracing::info!(
        event,
        action,
        repo,
        issue_number,
        "GitHub issue event received (task linking pending #29)"
    );

    Ok(())
}
