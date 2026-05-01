use super::helpers::fetch_active_project;
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Soft-delete a project. Caller must be Owner of the owning team.
pub async fn delete_project(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<(), ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_team_role(pool, project.team_id, caller_id, TeamRole::Owner).await?;

    sqlx::query!(
        "UPDATE projects SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        project_id
    )
    .execute(pool)
    .await?;

    Ok(())
}
