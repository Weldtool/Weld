use sqlx::PgPool;

use crate::errors::ApiError;

/// Handles `installation` and `installation_repositories` webhook events.
///
/// - `installation` / deleted: removes the `github_connections` row.
/// - `installation` / suspend|unsuspend: no-op (connection stays).
/// - `installation_repositories` / *: no-op. The set of accessible repos is
///   live-fetched from GitHub by `list_team_installation_repos` whenever a
///   project picker needs it, so there is no cached state to update here.
pub async fn handle(db: &PgPool, event: &str, payload: &serde_json::Value) -> Result<(), ApiError> {
    let installation_id = payload["installation"]["id"]
        .as_i64()
        .ok_or_else(|| ApiError::Validation("Missing installation.id".into()))?;

    let action = payload["action"].as_str().unwrap_or("");

    match event {
        "installation" => match action {
            "deleted" => {
                sqlx::query!(
                    "DELETE FROM github_connections WHERE installation_id = $1",
                    installation_id
                )
                .execute(db)
                .await?;
                tracing::info!(
                    installation_id,
                    "GitHub App uninstalled — connection removed"
                );
            }
            "created" => {
                tracing::info!(installation_id, action = "created", "Installation event");
            }
            other => {
                tracing::info!(installation_id, action = other, "Installation event");
            }
        },
        "installation_repositories" => {
            tracing::debug!(
                installation_id,
                action,
                "installation_repositories event — no cached state to update"
            );
        }
        _ => {}
    }

    Ok(())
}
