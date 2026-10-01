use super::helpers::{
    SLUG_ATTEMPTS, generate_unique_team_slug, is_unique_violation, require_team_role, slug_conflict,
};
use crate::errors::ApiError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub struct UpdatedTeam {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_id: Uuid,
    pub is_personal: bool,
    pub created_at: DateTime<Utc>,
}

/// Rename a team. Requires Admin or Owner role. Personal teams cannot be renamed.
/// Returns the updated team and the caller's role on it.
pub async fn update_team(
    pool: &PgPool,
    team_id: Uuid,
    user_id: Uuid,
    name: &str,
) -> Result<(UpdatedTeam, TeamRole), ApiError> {
    let role = require_team_role(pool, team_id, user_id, TeamRole::Admin).await?;

    let row = sqlx::query!(
        "SELECT is_personal FROM teams WHERE id = $1 AND deleted_at IS NULL",
        team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Team not found".into()))?;

    if row.is_personal {
        return Err(ApiError::Validation(
            "Cannot rename a personal workspace".into(),
        ));
    }

    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Validation("Team name cannot be empty".into()));
    }

    for _ in 0..SLUG_ATTEMPTS {
        let slug = generate_unique_team_slug(pool, trimmed, Some(team_id)).await?;

        let updated = sqlx::query!(
            r#"
            UPDATE teams SET name = $1, slug = $2
            WHERE id = $3 AND deleted_at IS NULL
            RETURNING id, name, slug, owner_id, is_personal, created_at
            "#,
            trimmed,
            slug,
            team_id
        )
        .fetch_one(pool)
        .await;

        let team = match updated {
            Ok(team) => team,
            // Lost the race for this slug; derive the next one.
            Err(e) if is_unique_violation(&e) => continue,
            Err(e) => return Err(e.into()),
        };

        return Ok((
            UpdatedTeam {
                id: team.id,
                name: team.name,
                slug: team.slug,
                owner_id: team.owner_id,
                is_personal: team.is_personal,
                created_at: team.created_at,
            },
            role,
        ));
    }

    Err(slug_conflict("team"))
}
