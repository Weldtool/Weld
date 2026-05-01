use super::helpers::require_team_role;
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Add a user to a team with the given role. Caller must be Admin or Owner.
pub async fn add_team_member(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
    new_member_id: Uuid,
    role: TeamRole,
) -> Result<(), ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Admin).await?;

    let role_str = match role {
        TeamRole::Owner => "owner",
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

    Ok(())
}
