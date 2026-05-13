use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::message::Channel;

pub struct CreateChannelInput {
    pub team_id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub topic: Option<String>,
    pub is_private: bool,
    pub is_direct: bool,
    pub initial_members: Vec<Uuid>,
}

pub async fn create_channel(
    pool: &PgPool,
    caller_id: Uuid,
    input: CreateChannelInput,
) -> Result<Channel, ApiError> {
    require_team_role(pool, input.team_id, caller_id, TeamRole::Member).await?;

    let name = input.name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Err(ApiError::Validation("Channel name cannot be empty".into()));
    }
    if name.len() > 80 {
        return Err(ApiError::Validation("Channel name too long".into()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ApiError::Validation(
            "Channel name may only contain letters, numbers, '-' and '_'".into(),
        ));
    }

    if let Some(pid) = input.project_id {
        let belongs = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM projects
                            WHERE id = $1 AND team_id = $2 AND deleted_at IS NULL)",
            pid,
            input.team_id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);
        if !belongs {
            return Err(ApiError::Validation(
                "Project does not belong to this team".into(),
            ));
        }
    }

    let mut tx = pool.begin().await?;

    let channel = sqlx::query_as!(
        Channel,
        r#"
        INSERT INTO channels (team_id, project_id, name, topic, is_direct, is_private, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, team_id, project_id, name, topic, is_direct, is_private,
                  created_by, created_at, updated_at, deleted_at,
                  github_repos as "github_repos!: Vec<String>",
                  github_event_filter
        "#,
        input.team_id,
        input.project_id,
        name,
        input.topic.as_deref(),
        input.is_direct,
        input.is_private,
        caller_id,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            ApiError::Conflict("A channel with this name already exists in the team".into())
        }
        _ => ApiError::from(e),
    })?;

    // creator is always a member (so private/DM channels work out of the box)
    sqlx::query!(
        "INSERT INTO channel_memberships (channel_id, user_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
        channel.id,
        caller_id
    )
    .execute(&mut *tx)
    .await?;

    for user_id in &input.initial_members {
        if *user_id == caller_id {
            continue;
        }
        // verify the invitee is on the team
        let on_team = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM team_memberships
                            WHERE team_id = $1 AND user_id = $2)",
            input.team_id,
            user_id
        )
        .fetch_one(&mut *tx)
        .await?
        .unwrap_or(false);
        if !on_team {
            return Err(ApiError::Validation(
                "Cannot add user that is not a team member".into(),
            ));
        }
        sqlx::query!(
            "INSERT INTO channel_memberships (channel_id, user_id) VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
            channel.id,
            user_id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(channel)
}
