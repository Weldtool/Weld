use super::helpers::{encrypt_bytes, exchange_oauth_code};
use crate::errors::ApiError;
use crate::state::GithubConfig;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

pub struct LinkedGithubUser {
    pub github_user_id: i64,
    pub github_login: String,
}

/// Exchanges a GitHub OAuth authorization code for a user access token and
/// links the GitHub identity to the given Weld user. The access token is
/// encrypted at rest with AES-256-GCM before storage.
#[instrument(skip(pool, github, master_key, code))]
pub async fn handle_oauth_callback(
    pool: &PgPool,
    github: &GithubConfig,
    master_key: &str,
    weld_user_id: Uuid,
    code: &str,
) -> Result<LinkedGithubUser, ApiError> {
    let (access_token, github_user_id, github_login) = exchange_oauth_code(github, code).await?;

    let (ciphertext, nonce) = encrypt_bytes(master_key, access_token.as_bytes())?;

    sqlx::query!(
        r#"
        INSERT INTO github_user_links
            (user_id, github_user_id, github_login, access_token_encrypted, nonce)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (user_id) DO UPDATE
            SET github_user_id          = EXCLUDED.github_user_id,
                github_login            = EXCLUDED.github_login,
                access_token_encrypted  = EXCLUDED.access_token_encrypted,
                nonce                   = EXCLUDED.nonce
        "#,
        weld_user_id,
        github_user_id,
        github_login,
        ciphertext,
        nonce,
    )
    .execute(pool)
    .await?;

    Ok(LinkedGithubUser {
        github_user_id,
        github_login,
    })
}
