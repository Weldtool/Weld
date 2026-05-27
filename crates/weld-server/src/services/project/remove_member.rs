use super::helpers::{fetch_active_project, require_project_team_role};
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Remove a user's explicit access to a project. Caller must be Admin+ on the
/// project's team. Removing a team Owner/Admin's row has no practical effect —
/// they retain implicit access to all team projects.
pub async fn remove_project_member(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
    member_id: Uuid,
) -> Result<(), ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Admin).await?;

    sqlx::query!(
        "DELETE FROM project_memberships WHERE project_id = $1 AND user_id = $2",
        project_id,
        member_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}
