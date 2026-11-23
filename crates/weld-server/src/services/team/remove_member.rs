use super::helpers::require_team_role;
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Remove a user from a team. Admin+ to remove another user; any member may
/// remove themselves (leave). The team owner cannot be removed — they must
/// transfer ownership first.
pub async fn remove_team_member(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
    target_user_id: Uuid,
) -> Result<(), ApiError> {
    // Self-removal is allowed for any member; otherwise require Admin+.
    if caller_id != target_user_id {
        require_team_role(pool, team_id, caller_id, TeamRole::Admin).await?;
    } else {
        // Make sure the caller is at least a Viewer (i.e. actually in the team)
        // so the error for "not a member" is consistent.
        require_team_role(pool, team_id, caller_id, TeamRole::Viewer).await?;
    }

    // Refuse to remove the team owner — they have to hand off first.
    let owner_id: Uuid = sqlx::query_scalar!(
        "SELECT owner_id FROM teams WHERE id = $1 AND deleted_at IS NULL",
        team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Team not found".into()))?;

    if target_user_id == owner_id {
        return Err(ApiError::Conflict(
            "Cannot remove the team owner. Transfer ownership first.".into(),
        ));
    }

    let mut tx = pool.begin().await?;

    let result = sqlx::query!(
        "DELETE FROM team_memberships WHERE team_id = $1 AND user_id = $2",
        team_id,
        target_user_id,
    )
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "User is not a member of this team".into(),
        ));
    }

    // Leaving the team ends access to everything inside it. Clearing the
    // per-project and per-channel rows too means re-adding someone later starts
    // them from scratch instead of silently restoring old private access.
    sqlx::query!(
        "DELETE FROM project_memberships
         WHERE user_id = $2
           AND project_id IN (SELECT id FROM projects WHERE team_id = $1)",
        team_id,
        target_user_id,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "DELETE FROM channel_memberships
         WHERE user_id = $2
           AND channel_id IN (SELECT id FROM channels WHERE team_id = $1)",
        team_id,
        target_user_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}
