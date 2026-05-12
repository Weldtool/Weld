use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Removes the GitHub App connection for a team. Requires Admin+ on the team.
#[instrument(skip(pool))]
pub async fn disconnect_github(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
) -> Result<(), ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Admin).await?;

    let result = sqlx::query!("DELETE FROM github_connections WHERE team_id = $1", team_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "No GitHub connection found for this team".into(),
        ));
    }

    Ok(())
}
