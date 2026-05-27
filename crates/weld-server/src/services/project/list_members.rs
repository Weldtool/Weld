use super::helpers::{fetch_active_project, require_project_team_role};
use crate::errors::ApiError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// A user explicitly granted access to a project, with the team role they hold.
pub struct ProjectMember {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub team_role: TeamRole,
    pub added_at: DateTime<Utc>,
}

/// List the explicit members of a project. Caller must have project access
/// (Viewer+). Note: team Owners/Admins have implicit access and may not appear
/// here unless they were also added explicitly.
pub async fn list_project_members(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<ProjectMember>, ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Viewer).await?;

    let rows = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.display_name, u.avatar_url,
               tm.role as "role: TeamRole",
               pm.added_at
        FROM project_memberships pm
        JOIN users u ON u.id = pm.user_id
        JOIN team_memberships tm ON tm.user_id = pm.user_id AND tm.team_id = $2
        WHERE pm.project_id = $1
        ORDER BY u.display_name ASC
        "#,
        project_id,
        project.team_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ProjectMember {
            user_id: r.id,
            email: r.email,
            display_name: r.display_name,
            avatar_url: r.avatar_url,
            team_role: r.role,
            added_at: r.added_at,
        })
        .collect())
}
