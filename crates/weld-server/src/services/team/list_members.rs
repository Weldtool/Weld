use crate::errors::ApiError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub struct TeamMember {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: TeamRole,
    pub joined_at: DateTime<Utc>,
}

/// List members of a team. Caller must be a Viewer+ on the team.
pub async fn list_team_members(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<TeamMember>, ApiError> {
    super::helpers::require_team_role(pool, team_id, caller_id, TeamRole::Viewer).await?;

    let rows = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.display_name, u.avatar_url,
               tm.role as "role: TeamRole",
               tm.joined_at
        FROM team_memberships tm
        JOIN users u ON u.id = tm.user_id
        WHERE tm.team_id = $1
        ORDER BY u.display_name ASC
        "#,
        team_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TeamMember {
            user_id: r.id,
            email: r.email,
            display_name: r.display_name,
            avatar_url: r.avatar_url,
            role: r.role,
            joined_at: r.joined_at,
        })
        .collect())
}
