use super::helpers::fetch_active_channel;
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::message::Channel;

#[derive(Default)]
pub struct UpdateChannelInput {
    pub name: Option<String>,
    pub topic: Option<Option<String>>,
}

pub async fn update_channel(
    pool: &PgPool,
    channel_id: Uuid,
    caller_id: Uuid,
    input: UpdateChannelInput,
) -> Result<Channel, ApiError> {
    let channel = fetch_active_channel(pool, channel_id).await?;
    // editing channel metadata requires Admin+ on the team
    require_team_role(pool, channel.team_id, caller_id, TeamRole::Admin).await?;

    let new_name = match &input.name {
        Some(n) => {
            let name = n.trim().to_ascii_lowercase();
            if name.is_empty() {
                return Err(ApiError::Validation("Channel name cannot be empty".into()));
            }
            if !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(ApiError::Validation(
                    "Channel name may only contain letters, numbers, '-' and '_'".into(),
                ));
            }
            name
        }
        None => channel.name.clone(),
    };
    let new_topic = match &input.topic {
        Some(v) => v.clone(),
        None => channel.topic.clone(),
    };

    let updated = sqlx::query_as!(
        Channel,
        r#"
        UPDATE channels
        SET name = $1, topic = $2, updated_at = NOW()
        WHERE id = $3 AND deleted_at IS NULL
        RETURNING id, team_id, project_id, name, topic, is_direct, is_private,
                  created_by, created_at, updated_at, deleted_at
        "#,
        new_name,
        new_topic.as_deref(),
        channel_id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            ApiError::Conflict("A channel with this name already exists in the team".into())
        }
        _ => ApiError::from(e),
    })?;

    Ok(updated)
}
