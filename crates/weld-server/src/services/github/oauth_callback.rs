use super::helpers::exchange_oauth_code;
use super::user_token::store_tokens;
use crate::errors::ApiError;
use crate::state::GithubConfig;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

pub struct LinkedGithubUser {
    pub github_user_id: i64,
    pub github_login: String,
}

/// Exchanges an authorization code for a GitHub App user-to-server token and
/// links the GitHub identity to the given Weld user.
///
/// Both the access and refresh tokens are encrypted at rest with AES-256-GCM.
/// The token is what lets Weld act *as this person* on GitHub, so reviews and
/// merges they trigger carry their name rather than the bot's.
#[instrument(skip(pool, github, master_key, code))]
pub async fn handle_oauth_callback(
    pool: &PgPool,
    github: &GithubConfig,
    master_key: &str,
    weld_user_id: Uuid,
    code: &str,
) -> Result<LinkedGithubUser, ApiError> {
    let (token, github_user_id, github_login) = exchange_oauth_code(github, code).await?;

    // Insert the identity first with placeholder secrets, then let `store_tokens`
    // write the encrypted pair — one code path owns the encryption, so the
    // callback and the refresh path can never drift apart.
    sqlx::query!(
        r#"
        INSERT INTO github_user_links
            (user_id, github_user_id, github_login, access_token_encrypted, nonce)
        VALUES ($1, $2, $3, '\x'::bytea, '\x'::bytea)
        ON CONFLICT (user_id) DO UPDATE
            SET github_user_id = EXCLUDED.github_user_id,
                github_login   = EXCLUDED.github_login
        "#,
        weld_user_id,
        github_user_id,
        github_login,
    )
    .execute(pool)
    .await?;

    store_tokens(pool, master_key, weld_user_id, &token).await?;

    Ok(LinkedGithubUser {
        github_user_id,
        github_login,
    })
}
