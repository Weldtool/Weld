use super::connect::GithubConnectionRecord;
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Fetches the GitHub connection for a team. Requires Viewer+ on the team.
#[instrument(skip(pool))]
pub async fn get_github_connection(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
) -> Result<GithubConnectionRecord, ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Viewer).await?;

    let row = sqlx::query!(
        r#"
        SELECT id, team_id, installation_id, account_login, repos, created_by,
               created_at, updated_at
        FROM github_connections
        WHERE team_id = $1
        "#,
        team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("No GitHub connection found for this team".into()))?;

    Ok(GithubConnectionRecord {
        id: row.id,
        team_id: row.team_id,
        installation_id: row.installation_id,
        account_login: row.account_login,
        repos: row.repos,
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
