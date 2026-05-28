use super::helpers::require_team_role;
use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Soft-delete a team. Requires Owner role. Personal teams cannot be deleted.
pub async fn delete_team(pool: &PgPool, team_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    require_team_role(pool, team_id, user_id, TeamRole::Owner).await?;

    let row = sqlx::query!(
        "SELECT is_personal FROM teams WHERE id = $1 AND deleted_at IS NULL",
        team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Team not found".into()))?;

    if row.is_personal {
        return Err(ApiError::Validation(
            "Cannot delete your personal workspace".into(),
        ));
    }

    sqlx::query!(
        "UPDATE teams SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        team_id
    )
    .execute(pool)
    .await?;

    Ok(())
}
