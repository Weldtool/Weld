use super::helpers::{fetch_active_project, require_project_team_role};
use super::list_members::ProjectMember;
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Add a user to a project, granting them access. Caller must be Admin+ on the
/// project's team. The target must already be a member of that team — projects
/// can only be shared with people who belong to the owning team.
pub async fn add_project_member(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
    new_member_id: Uuid,
) -> Result<ProjectMember, ApiError> {
    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Admin).await?;

    let team_role = sqlx::query_scalar!(
        r#"SELECT role as "role: TeamRole" FROM team_memberships WHERE team_id = $1 AND user_id = $2"#,
        project.team_id,
        new_member_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        ApiError::Validation("User must be a member of the team before joining a project".into())
    })?;

    let result = sqlx::query!(
        "INSERT INTO project_memberships (project_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        project_id,
        new_member_id,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "User is already a project member".into(),
        ));
    }

    let row = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.display_name, u.avatar_url, pm.added_at
        FROM project_memberships pm
        JOIN users u ON u.id = pm.user_id
        WHERE pm.project_id = $1 AND pm.user_id = $2
        "#,
        project_id,
        new_member_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(ProjectMember {
        user_id: row.id,
        email: row.email,
        display_name: row.display_name,
        avatar_url: row.avatar_url,
        team_role,
        added_at: row.added_at,
    })
}
