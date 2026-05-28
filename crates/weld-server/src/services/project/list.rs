use crate::errors::ApiError;
use crate::services::team::helpers::{require_team_role, role_level};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::project::Project;

/// List active projects for a team that the caller may see.
///
/// Team Owners/Admins see every project; Members/Viewers see only the projects
/// they've been explicitly added to (`project_memberships`).
pub async fn list_team_projects(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<Project>, ApiError> {
    let role = require_team_role(pool, team_id, caller_id, TeamRole::Viewer).await?;

    let projects = if role_level(&role) >= role_level(&TeamRole::Admin) {
        sqlx::query_as!(
            Project,
            r#"
            SELECT id, team_id, name, slug, description, github_repo, done_column_id,
                   in_progress_column_id, review_column_id, auto_close_on_issue_closed,
                   created_at, updated_at, deleted_at
            FROM projects
            WHERE team_id = $1 AND deleted_at IS NULL
            ORDER BY created_at ASC
            "#,
            team_id
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            Project,
            r#"
            SELECT p.id, p.team_id, p.name, p.slug, p.description, p.github_repo,
                   p.done_column_id, p.in_progress_column_id, p.review_column_id,
                   p.auto_close_on_issue_closed, p.created_at, p.updated_at, p.deleted_at
            FROM projects p
            JOIN project_memberships pm ON pm.project_id = p.id AND pm.user_id = $2
            WHERE p.team_id = $1 AND p.deleted_at IS NULL
            ORDER BY p.created_at ASC
            "#,
            team_id,
            caller_id
        )
        .fetch_all(pool)
        .await?
    };

    Ok(projects)
}
