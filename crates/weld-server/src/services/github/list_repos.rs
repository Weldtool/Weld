use super::helpers::{create_installation_token, list_installation_repos};
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use crate::state::GithubConfig;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Returns the repositories accessible via the team's GitHub App installation.
///
/// Live-fetched from GitHub each call — there is no cached `repos` column on
/// `github_connections`. Requires Viewer+ on the team so any member can
/// populate per-project repo dropdowns.
#[instrument(skip(pool, github))]
pub async fn list_team_installation_repos(
    pool: &PgPool,
    github: &GithubConfig,
    team_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<String>, ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Viewer).await?;

    let installation_id = sqlx::query_scalar!(
        "SELECT installation_id FROM github_connections WHERE team_id = $1",
        team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("No GitHub connection found for this team".into()))?;

    let token = create_installation_token(github, installation_id).await?;
    list_installation_repos(&token).await
}
