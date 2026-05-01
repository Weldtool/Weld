use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;

/// List active projects for a team. Caller must be a member of the team.
pub async fn list_team_projects(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<Project>, ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Viewer).await?;

    let projects = sqlx::query_as!(
        Project,
        r#"
        SELECT id, team_id, name, slug, description, github_repo,
               created_at, updated_at, deleted_at
        FROM projects
        WHERE team_id = $1 AND deleted_at IS NULL
        ORDER BY created_at ASC
        "#,
        team_id
    )
    .fetch_all(pool)
    .await?;

    Ok(projects)
}
