use super::helpers::require_team_role;
use super::list_members::TeamMember;
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Resolves an email to a user ID. Case-insensitive match. Returns 404 if
/// no user with that email is registered.
pub async fn find_user_id_by_email(pool: &PgPool, email: &str) -> Result<Uuid, ApiError> {
    sqlx::query_scalar!("SELECT id FROM users WHERE LOWER(email) = LOWER($1)", email)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("No user is registered with email {email}")))
}

/// Add a user to a team with the given role. Caller must be Admin or Owner.
/// Returns the newly-joined member's full row so callers can append directly
/// to UI lists without a follow-up fetch.
pub async fn add_team_member(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
    new_member_id: Uuid,
    role: TeamRole,
) -> Result<TeamMember, ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Admin).await?;

    let team = sqlx::query!(
        "SELECT is_personal FROM teams WHERE id = $1 AND deleted_at IS NULL",
        team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Team not found".into()))?;

    // A personal workspace is private to its owner by definition.
    if team.is_personal {
        return Err(ApiError::Validation(
            "Cannot add members to a personal workspace".into(),
        ));
    }

    let role_str = match role {
        // A team has exactly one Owner (`teams.owner_id`). Letting an Admin mint
        // another would hand out the one role that can delete the team.
        TeamRole::Owner => {
            return Err(ApiError::Validation(
                "Members cannot be added as Owner".into(),
            ));
        }
        TeamRole::Admin => "admin",
        TeamRole::Member => "member",
        TeamRole::Viewer => "viewer",
    };

    let result = sqlx::query!(
        r#"
        INSERT INTO team_memberships (team_id, user_id, role)
        VALUES ($1, $2, $3::team_role)
        ON CONFLICT (user_id, team_id) DO NOTHING
        "#,
        team_id,
        new_member_id,
        role_str as _
    )
    .execute(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
            ApiError::NotFound("User not found".into())
        }
        _ => ApiError::Internal(format!("Database error: {e}")),
    })?;

    if result.rows_affected() == 0 {
        return Err(ApiError::Conflict("User is already a team member".into()));
    }

    let row = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.display_name, u.avatar_url,
               tm.role as "role: TeamRole",
               tm.joined_at
        FROM team_memberships tm
        JOIN users u ON u.id = tm.user_id
        WHERE tm.team_id = $1 AND tm.user_id = $2
        "#,
        team_id,
        new_member_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(TeamMember {
        user_id: row.id,
        email: row.email,
        display_name: row.display_name,
        avatar_url: row.avatar_url,
        role: row.role,
        joined_at: row.joined_at,
    })
}
