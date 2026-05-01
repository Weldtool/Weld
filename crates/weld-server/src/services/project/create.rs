use super::helpers::generate_unique_project_slug;
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;

/// Create a project under the given team. Caller must be Admin or Owner of the team.
pub async fn create_project(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
    name: &str,
    description: Option<&str>,
) -> Result<Project, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Validation("Project name cannot be empty".into()));
    }

    require_team_role(pool, team_id, caller_id, TeamRole::Admin).await?;

    let slug = generate_unique_project_slug(pool, trimmed).await?;

    let project = sqlx::query_as!(
        Project,
        r#"
        INSERT INTO projects (team_id, name, slug, description)
        VALUES ($1, $2, $3, $4)
        RETURNING id, team_id, name, slug, description, github_repo,
                  created_at, updated_at, deleted_at
        "#,
        team_id,
        trimmed,
        slug,
        description
    )
    .fetch_one(pool)
    .await?;

    Ok(project)
}
