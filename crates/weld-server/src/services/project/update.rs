use super::helpers::{fetch_active_project, require_project_team_role};
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;

#[derive(Default)]
pub struct UpdateProjectInput {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub github_repo: Option<Option<String>>,
}

/// Update a project. Caller must be Admin or Owner of the owning team.
pub async fn update_project(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
    input: UpdateProjectInput,
) -> Result<Project, ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Admin).await?;

    if let Some(name) = &input.name
        && name.trim().is_empty()
    {
        return Err(ApiError::Validation("Project name cannot be empty".into()));
    }

    let new_name = input
        .name
        .as_ref()
        .map(|n| n.trim().to_string())
        .unwrap_or_else(|| project.name.clone());

    let new_description = match &input.description {
        Some(v) => v.clone(),
        None => project.description.clone(),
    };
    let new_github_repo = match &input.github_repo {
        Some(v) => v.clone(),
        None => project.github_repo.clone(),
    };

    let updated = sqlx::query_as!(
        Project,
        r#"
        UPDATE projects
        SET name = $1,
            description = $2,
            github_repo = $3,
            updated_at = NOW()
        WHERE id = $4 AND deleted_at IS NULL
        RETURNING id, team_id, name, slug, description, github_repo,
                  created_at, updated_at, deleted_at
        "#,
        new_name,
        new_description,
        new_github_repo,
        project_id
    )
    .fetch_one(pool)
    .await?;

    Ok(updated)
}
