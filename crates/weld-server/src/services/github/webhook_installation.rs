use sqlx::PgPool;

use crate::errors::ApiError;

/// Handles `installation` and `installation_repositories` webhook events.
///
/// - `installation` / deleted: removes the `github_connections` row.
/// - `installation` / suspend|unsuspend: no-op (connection stays).
/// - `installation_repositories` / added: merges new repos into the stored list.
/// - `installation_repositories` / removed: prunes repos from the stored list.
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
            let row = sqlx::query!(
                "SELECT repos FROM github_connections WHERE installation_id = $1",
                installation_id
            )
            .fetch_optional(db)
            .await?;

            let Some(row) = row else {
                tracing::warn!(
                    installation_id,
                    "installation_repositories event for unknown installation — ignoring"
                );
                return Ok(());
            };

            let mut repos = row.repos;

            match action {
                "added" => {
                    let added: Vec<String> = payload["repositories_added"]
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|r| r["full_name"].as_str().map(str::to_string))
                        .collect();
                    for repo in &added {
                        if !repos.contains(repo) {
                            repos.push(repo.clone());
                        }
                    }
                    tracing::info!(installation_id, added = added.len(), "Repos added");
                }
                "removed" => {
                    let removed: Vec<String> = payload["repositories_removed"]
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|r| r["full_name"].as_str().map(str::to_string))
                        .collect();
                    repos.retain(|r| !removed.contains(r));
                    tracing::info!(installation_id, removed = removed.len(), "Repos removed");
                }
                other => {
                    tracing::debug!(
                        installation_id,
                        action = other,
                        "Unhandled repositories action"
                    );
                    return Ok(());
                }
            }

            sqlx::query!(
                "UPDATE github_connections SET repos = $2, updated_at = NOW() WHERE installation_id = $1",
                installation_id,
                &repos,
            )
            .execute(db)
            .await?;
        }
        _ => {}
    }

    Ok(())
}
