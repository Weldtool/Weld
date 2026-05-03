use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::message::Channel;

/// List channels in a team that the caller can see:
/// - all public, non-DM channels in the team
/// - private channels and DMs the caller is a member of
pub async fn list_channels(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<Channel>, ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Viewer).await?;

    let rows = sqlx::query_as!(
        Channel,
        r#"
        SELECT id, team_id, project_id, name, topic, is_direct, is_private,
               created_by, created_at, updated_at, deleted_at
        FROM channels
        WHERE team_id = $1
          AND deleted_at IS NULL
          AND (
            (is_private = FALSE AND is_direct = FALSE)
            OR EXISTS (
                SELECT 1 FROM channel_memberships cm
                WHERE cm.channel_id = channels.id AND cm.user_id = $2
            )
          )
        ORDER BY is_direct ASC, name ASC
        "#,
        team_id,
        caller_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
