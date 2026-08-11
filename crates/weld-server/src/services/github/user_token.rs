//! Acting on GitHub as the Weld user who asked, rather than as the App.
//!
//! Every write used to go through the installation token, so GitHub recorded
//! the Weld bot as the author of every review, merge, comment and issue. That
//! erases the audit trail exactly where it matters most — "who approved this?"
//! is the question a PR history exists to answer.
//!
//! When a user has linked their GitHub account we use their own user-to-server
//! token. When they have not, we fall back to the installation token so nothing
//! breaks; callers can tell the two apart and adjust attribution.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::state::GithubConfig;

use super::helpers::{
    UserToken, create_installation_token, decrypt_bytes, encrypt_bytes, refresh_user_token,
};

/// A token plus who GitHub will attribute the resulting action to.
pub struct ActingToken {
    pub token: String,
    /// True when acting as the user themselves; false when falling back to the
    /// App installation.
    pub as_user: bool,
}

/// Refresh a little before actual expiry so a request cannot die in flight.
const REFRESH_LEEWAY: Duration = Duration::minutes(5);

/// Returns the token to act with for `user_id`, preferring their own.
///
/// Falls back to the installation token whenever the user has no usable link —
/// not linked, no refresh token, or a refresh GitHub rejected. A failure to act
/// as the user should degrade attribution, never block the work.
pub async fn acting_token(
    pool: &PgPool,
    github: &GithubConfig,
    master_key: Option<&str>,
    user_id: Uuid,
    installation_id: i64,
) -> Result<ActingToken, ApiError> {
    if let Some(master_key) = master_key {
        match user_access_token(pool, github, master_key, user_id).await {
            Ok(Some(token)) => {
                return Ok(ActingToken {
                    token,
                    as_user: true,
                });
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(
                    user_id = %user_id,
                    error = %e,
                    "Could not use the user's GitHub token; acting as the app instead"
                );
            }
        }
    }

    Ok(ActingToken {
        token: create_installation_token(github, installation_id).await?,
        as_user: false,
    })
}

/// Returns a currently-valid access token for `user_id`, refreshing if needed.
/// `None` means the user has not linked their GitHub account.
async fn user_access_token(
    pool: &PgPool,
    github: &GithubConfig,
    master_key: &str,
    user_id: Uuid,
) -> Result<Option<String>, ApiError> {
    let row = sqlx::query!(
        r#"
        SELECT access_token_encrypted, nonce, token_expires_at,
               refresh_token_encrypted, refresh_nonce, refresh_expires_at
        FROM github_user_links
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    // A token with no recorded expiry never expires (the App has token
    // expiration turned off), so it is good as-is.
    let still_valid = match row.token_expires_at {
        Some(expires_at) => expires_at > Utc::now() + REFRESH_LEEWAY,
        None => true,
    };

    if still_valid {
        let plain = decrypt_bytes(master_key, &row.access_token_encrypted, &row.nonce)?;
        return Ok(Some(String::from_utf8_lossy(&plain).into_owned()));
    }

    // Expired. Refresh if we can, otherwise the link is dead and the caller
    // falls back to the installation token.
    let (Some(refresh_ct), Some(refresh_nonce)) = (row.refresh_token_encrypted, row.refresh_nonce)
    else {
        return Ok(None);
    };

    if row.refresh_expires_at.is_some_and(|e| e <= Utc::now()) {
        tracing::info!(user_id = %user_id, "GitHub refresh token expired; re-link required");
        return Ok(None);
    }

    let refresh_plain = decrypt_bytes(master_key, &refresh_ct, &refresh_nonce)?;
    let refresh = String::from_utf8_lossy(&refresh_plain).into_owned();

    let refreshed = refresh_user_token(github, &refresh).await?;
    let access = refreshed.access_token.clone();
    store_tokens(pool, master_key, user_id, &refreshed).await?;

    Ok(Some(access))
}

/// Encrypts and persists a user's token pair. Shared by the OAuth callback and
/// the refresh path so both write the same shape.
pub async fn store_tokens(
    pool: &PgPool,
    master_key: &str,
    user_id: Uuid,
    token: &UserToken,
) -> Result<(), ApiError> {
    let (access_ct, access_nonce) = encrypt_bytes(master_key, token.access_token.as_bytes())?;
    let refresh = token
        .refresh_token
        .as_deref()
        .map(|r| encrypt_bytes(master_key, r.as_bytes()))
        .transpose()?;
    let (refresh_ct, refresh_nonce) = match refresh {
        Some((ct, n)) => (Some(ct), Some(n)),
        None => (None, None),
    };

    sqlx::query!(
        r#"
        UPDATE github_user_links
        SET access_token_encrypted  = $2,
            nonce                   = $3,
            refresh_token_encrypted = $4,
            refresh_nonce           = $5,
            token_expires_at        = $6,
            refresh_expires_at      = $7
        WHERE user_id = $1
        "#,
        user_id,
        access_ct,
        access_nonce,
        refresh_ct,
        refresh_nonce,
        token.expires_at,
        token.refresh_expires_at,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Convenience for the common shape: look up a team's installation, then decide
/// which token to act with.
pub async fn acting_token_for_team(
    pool: &PgPool,
    github: &GithubConfig,
    master_key: Option<&str>,
    user_id: Uuid,
    team_id: Uuid,
) -> Result<ActingToken, ApiError> {
    let installation_id = sqlx::query_scalar!(
        "SELECT installation_id FROM github_connections WHERE team_id = $1",
        team_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::Validation("Team has no GitHub connection".into()))?;

    acting_token(pool, github, master_key, user_id, installation_id).await
}

/// Human-readable footer marking an action that could not be attributed to a
/// real GitHub account. Returns `None` when the user acted as themselves, in
/// which case GitHub's own authorship is the record and no footer is needed.
pub fn attribution_footer(acting: &ActingToken, display_name: &str) -> Option<String> {
    (!acting.as_user).then(|| format!("_by **{display_name}** via Weld_"))
}

/// Token expiry maths is the part most likely to be wrong in a way nothing
/// notices until a user is mysteriously acting as the bot again.
#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the freshness check in `user_access_token`.
    fn is_fresh(expires_at: Option<chrono::DateTime<Utc>>) -> bool {
        match expires_at {
            Some(e) => e > Utc::now() + REFRESH_LEEWAY,
            None => true,
        }
    }

    #[test]
    fn no_expiry_means_never_refresh() {
        assert!(is_fresh(None));
    }

    #[test]
    fn token_well_inside_its_life_is_fresh() {
        assert!(is_fresh(Some(Utc::now() + Duration::hours(8))));
    }

    #[test]
    fn already_expired_token_is_stale() {
        assert!(!is_fresh(Some(Utc::now() - Duration::minutes(1))));
    }

    #[test]
    fn token_inside_the_leeway_refreshes_early() {
        // Still technically valid, but not for long enough to trust with a
        // request that is about to go out.
        assert!(!is_fresh(Some(Utc::now() + Duration::minutes(2))));
    }
}
