use super::helpers::get_installation_account;
use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use crate::state::GithubConfig;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub struct GithubConnectionRecord {
    pub id: Uuid,
    pub team_id: Uuid,
    pub installation_id: i64,
    pub account_login: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Stores (or updates) a GitHub App installation for a team.
///
/// Fetches the account login from GitHub and upserts a row in
/// `github_connections`. The set of accessible repositories is intentionally
/// not cached — it is fetched live from GitHub by `list_team_installation_repos`
/// when needed. Requires Admin+ on the team.
#[instrument(skip(pool, github))]
pub async fn connect_github(
    pool: &PgPool,
    github: &GithubConfig,
    team_id: Uuid,
    caller_id: Uuid,
    installation_id: i64,
) -> Result<GithubConnectionRecord, ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Admin).await?;

    let account_login = get_installation_account(github, installation_id).await?;

    let row = sqlx::query!(
        r#"
        INSERT INTO github_connections (team_id, installation_id, account_login, created_by)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (team_id) DO UPDATE
            SET installation_id = EXCLUDED.installation_id,
                account_login   = EXCLUDED.account_login,
                updated_at      = NOW()
        RETURNING id, team_id, installation_id, account_login, created_by,
                  created_at, updated_at
        "#,
        team_id,
        installation_id,
        account_login,
        caller_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(GithubConnectionRecord {
        id: row.id,
        team_id: row.team_id,
        installation_id: row.installation_id,
        account_login: row.account_login,
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
