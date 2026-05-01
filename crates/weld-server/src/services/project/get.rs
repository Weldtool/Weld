use super::helpers::{fetch_active_project, require_project_team_role};
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;

/// Fetch a project by id. Caller must be a member of the owning team.
pub async fn get_project(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Project, ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Viewer).await?;
    Ok(project)
}
