use super::helpers::{fetch_active_channel, is_channel_member};
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub async fn add_channel_member(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
    new_member_id: Uuid,
) -> Result<(), ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;

    // DMs are 1:1 by definition; their membership must not grow.
    if channel.is_direct {
        return Err(ApiError::Forbidden(
            "Cannot add members to a direct message channel".into(),
        ));
    }

    // adding members requires Member+ on the team
    require_team_role(pool, channel.team_id, caller_id, TeamRole::Member).await?;

    // For private channels, the caller must already be a member of the
    // channel — otherwise any team member could self-grant access to a
    // private conversation they were never party to.
    if channel.is_private && !is_channel_member(pool, channel.id, caller_id).await? {
        return Err(ApiError::Forbidden(
            "Only existing members can add others to a private channel".into(),
        ));
    }

    let on_team = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM team_memberships
                        WHERE team_id = $1 AND user_id = $2)",
        channel.team_id,
        new_member_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if !on_team {
        return Err(ApiError::Validation(
            "User is not a member of this team".into(),
        ));
    }

    let result = sqlx::query!(
        "INSERT INTO channel_memberships (channel_id, user_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
        channel_id,
        new_member_id
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "User is already a member of this channel".into(),
        ));
    }
    Ok(())
}
