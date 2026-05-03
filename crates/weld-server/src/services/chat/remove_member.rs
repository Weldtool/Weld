use super::helpers::fetch_active_channel;
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub async fn remove_channel_member(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
    target_user_id: Uuid,
) -> Result<(), ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;
    // self-removal is allowed; otherwise Admin+ on the team is required
    if caller_id != target_user_id {
        require_team_role(pool, channel.team_id, caller_id, TeamRole::Admin).await?;
    } else {
        require_team_role(pool, channel.team_id, caller_id, TeamRole::Viewer).await?;
    }

    let result = sqlx::query!(
        "DELETE FROM channel_memberships WHERE channel_id = $1 AND user_id = $2",
        channel_id,
        target_user_id
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "User is not a member of this channel".into(),
        ));
    }
    Ok(())
}
