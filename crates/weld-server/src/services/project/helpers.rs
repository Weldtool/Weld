use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;

pub async fn generate_unique_project_slug(pool: &PgPool, name: &str) -> Result<String, ApiError> {
    let base = slug::slugify(name);
    let base = if base.is_empty() {
        "project".to_string()
    } else {
        base
    };
    let mut candidate = base.clone();
    let mut count = 1;
    loop {
        let exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE slug = $1 AND deleted_at IS NULL)",
            candidate
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);
        if !exists {
            break;
        }
        count += 1;
        if count > 1000 {
            return Err(ApiError::Internal("Failed to generate unique slug".into()));
        }
        candidate = format!("{}-{}", base, count);
    }
    Ok(candidate)
}

/// Fetch an active (non-deleted) project by id, or return NotFound.
pub async fn fetch_active_project(pool: &PgPool, id: Uuid) -> Result<Project, ApiError> {
    sqlx::query_as!(
        Project,
        r#"
        SELECT id, team_id, name, slug, description, github_repo, done_column_id, auto_close_on_issue_closed,
               created_at, updated_at, deleted_at
        FROM projects
        WHERE id = $1 AND deleted_at IS NULL
        "#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Project not found".into()))
}

/// Verify the user has at least `min_role` on the team that owns the project.
pub async fn require_project_team_role(
    pool: &PgPool,
    project: &Project,
    user_id: Uuid,
    min_role: TeamRole,
) -> Result<(), ApiError> {
    crate::services::team::helpers::require_team_role(pool, project.team_id, user_id, min_role)
        .await?;
    Ok(())
}
