use super::helpers::{
    SLUG_ATTEMPTS, generate_unique_team_slug, is_unique_violation, slug_conflict,
};
use crate::errors::ApiError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub struct CreatedTeam {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Create a new team and add the creator as Owner.
pub async fn create_team(
    pool: &PgPool,
    owner_id: Uuid,
    name: &str,
) -> Result<CreatedTeam, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Validation("Team name cannot be empty".into()));
    }

    for _ in 0..SLUG_ATTEMPTS {
        let slug = generate_unique_team_slug(pool, trimmed, None).await?;

        let mut tx = pool.begin().await?;

        let inserted = sqlx::query!(
            r#"
            INSERT INTO teams (name, slug, owner_id)
            VALUES ($1, $2, $3)
            RETURNING id, name, slug, owner_id, created_at
            "#,
            trimmed,
            slug,
            owner_id
        )
        .fetch_one(&mut *tx)
        .await;

        let team = match inserted {
            Ok(team) => team,
            // Lost the race for this slug; dropping `tx` rolls back. Try the next.
            Err(e) if is_unique_violation(&e) => continue,
            Err(e) => return Err(e.into()),
        };

        sqlx::query!(
            r#"
            INSERT INTO team_memberships (team_id, user_id, role)
            VALUES ($1, $2, 'owner')
            "#,
            team.id,
            owner_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        return Ok(CreatedTeam {
            id: team.id,
            name: team.name,
            slug: team.slug,
            owner_id: team.owner_id,
            created_at: team.created_at,
        });
    }

    Err(slug_conflict("team"))
}
