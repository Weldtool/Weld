use crate::errors::ApiError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub struct UserTeam {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_id: Uuid,
    pub role: TeamRole,
    pub created_at: DateTime<Utc>,
}

/// List all active teams the user is a member of, with the caller's role on each.
pub async fn list_user_teams(pool: &PgPool, user_id: Uuid) -> Result<Vec<UserTeam>, ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.name, t.slug, t.owner_id, t.created_at,
               tm.role as "role: TeamRole"
        FROM teams t
        JOIN team_memberships tm ON tm.team_id = t.id
        WHERE tm.user_id = $1 AND t.deleted_at IS NULL
        ORDER BY t.created_at ASC
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| UserTeam {
            id: r.id,
            name: r.name,
            slug: r.slug,
            owner_id: r.owner_id,
            role: r.role,
            created_at: r.created_at,
        })
        .collect())
}
