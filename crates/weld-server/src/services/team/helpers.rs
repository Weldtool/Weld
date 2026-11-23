use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// How many times a create/rename re-derives its slug after losing a race.
///
/// Slugs are picked with a check-then-insert, so two requests for the same name
/// can both choose the same "free" slug. The unique index catches the loser,
/// which then simply derives the next free slug instead of failing with a 500.
pub const SLUG_ATTEMPTS: usize = 8;

/// Whether `err` is a unique-constraint violation (here: a slug just taken).
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.is_unique_violation())
}

pub fn slug_conflict(kind: &str) -> ApiError {
    ApiError::Conflict(format!(
        "Could not reserve a unique URL for this {kind}. Please try again."
    ))
}

pub fn slugify(name: &str) -> String {
    slug::slugify(name)
}

/// Derives a slug for `name` that no other active team uses.
///
/// `exclude` is the team being renamed, if any: its own current slug must not
/// count as a collision, or renaming a team to the name it already has (or
/// changing only its capitalisation) bumps the slug to `name-2`.
pub async fn generate_unique_team_slug(
    pool: &PgPool,
    name: &str,
    exclude: Option<Uuid>,
) -> Result<String, ApiError> {
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
            "SELECT EXISTS(SELECT 1 FROM teams
                           WHERE slug = $1 AND deleted_at IS NULL
                             AND ($2::uuid IS NULL OR id <> $2))",
            candidate,
            exclude
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
pub fn role_level(role: &TeamRole) -> u8 {
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
