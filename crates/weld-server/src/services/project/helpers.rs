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
        SELECT id, team_id, name, slug, description, github_repo, done_column_id,
               in_progress_column_id, review_column_id, auto_close_on_issue_closed,
               github_comment_on_move,
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

/// Verify the user may act on `project` with at least `min_role`.
///
/// Access has two layers:
/// 1. The user must hold at least `min_role` on the project's team.
/// 2. Team Owners/Admins implicitly access every project in the team. Members
///    and Viewers must additionally be explicit project members
///    (`project_memberships`) — being on the team is no longer enough to see a
///    project.
pub async fn require_project_team_role(
    pool: &PgPool,
    project: &Project,
    user_id: Uuid,
    min_role: TeamRole,
) -> Result<(), ApiError> {
    let role =
        crate::services::team::helpers::require_team_role(pool, project.team_id, user_id, min_role)
            .await?;

    // Owners/Admins manage the whole team and don't need per-project rows.
    if crate::services::team::helpers::role_level(&role)
        >= crate::services::team::helpers::role_level(&TeamRole::Admin)
    {
        return Ok(());
    }

    let is_member = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM project_memberships WHERE project_id = $1 AND user_id = $2)",
        project.id,
        user_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);

    if is_member {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "You don't have access to this project".into(),
        ))
    }
}
