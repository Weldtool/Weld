use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub fn slugify(name: &str) -> String {
    slug::slugify(name)
}

pub async fn generate_unique_team_slug(pool: &PgPool, name: &str) -> Result<String, ApiError> {
    let base = slugify(name);
    let base = if base.is_empty() {
        "team".to_string()
    } else {
        base
    };
    let mut candidate = base.clone();
    let mut count = 1;
    loop {
        let exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM teams WHERE slug = $1 AND deleted_at IS NULL)",
            candidate
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);
        if !exists {
            break;
        }
        count += 1;
        if count > 1000 {
            return Err(ApiError::Internal("Failed to generate unique slug".into()));
        }
        candidate = format!("{}-{}", base, count);
    }
    Ok(candidate)
}

/// Numeric privilege level: higher = more permissions.
fn role_level(role: &TeamRole) -> u8 {
    match role {
        TeamRole::Owner => 4,
        TeamRole::Admin => 3,
        TeamRole::Member => 2,
        TeamRole::Viewer => 1,
    }
}

/// Verify the user has at least `min_role` on the team. Returns their actual role.
pub async fn require_team_role(
    pool: &PgPool,
    team_id: Uuid,
    user_id: Uuid,
    min_role: TeamRole,
) -> Result<TeamRole, ApiError> {
    let team_active = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM teams WHERE id = $1 AND deleted_at IS NULL)",
        team_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if !team_active {
        return Err(ApiError::NotFound("Team not found".into()));
    }

    let role = sqlx::query_scalar!(
        r#"SELECT role as "role: TeamRole" FROM team_memberships WHERE team_id = $1 AND user_id = $2"#,
        team_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    match role {
        Some(r) if role_level(&r) >= role_level(&min_role) => Ok(r),
        _ => Err(ApiError::Forbidden("Insufficient team permissions".into())),
    }
}
