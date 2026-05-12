use aes_gcm::{
    Aes256Gcm, Key,
    aead::{Aead, AeadCore, KeyInit},
};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tracing::instrument;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::state::GithubConfig;

// ── App JWT ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AppClaims {
    iat: i64,
    exp: i64,
    iss: String,
}

/// Creates a short-lived GitHub App JWT for authenticating as the GitHub App.
pub fn create_app_jwt(github: &GithubConfig) -> Result<String, ApiError> {
    let now = Utc::now().timestamp();
    let claims = AppClaims {
        iat: now - 60, // 60s leeway for clock skew
        exp: now + 540,
        iss: github.app_id.to_string(),
    };
    let key = EncodingKey::from_rsa_pem(&github.app_private_key_pem)
        .map_err(|e| ApiError::Internal(format!("Invalid GitHub App private key: {e}")))?;
    encode(&Header::new(Algorithm::RS256), &claims, &key)
        .map_err(|e| ApiError::Internal(format!("Failed to sign App JWT: {e}")))
}

// ── Installation token ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct InstallationTokenResponse {
    pub token: String,
}

/// Mints a short-lived installation access token for `installation_id`.
#[instrument(skip(github))]
pub async fn create_installation_token(
    github: &GithubConfig,
    installation_id: i64,
) -> Result<String, ApiError> {
    let jwt = create_app_jwt(github)?;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "https://api.github.com/app/installations/{installation_id}/access_tokens"
        ))
        .bearer_auth(&jwt)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub installation token request failed ({status}): {body}"
        )));
    }

    let token_resp: InstallationTokenResponse = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse installation token: {e}")))?;

    Ok(token_resp.token)
}

// ── Installation account ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct InstallationAccount {
    pub login: String,
}

#[derive(Deserialize)]
struct InstallationResponse {
    pub account: InstallationAccount,
}

/// Returns the GitHub account login that owns `installation_id`.
#[instrument(skip(github))]
pub async fn get_installation_account(
    github: &GithubConfig,
    installation_id: i64,
) -> Result<String, ApiError> {
    let jwt = create_app_jwt(github)?;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "https://api.github.com/app/installations/{installation_id}"
        ))
        .bearer_auth(&jwt)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub get-installation request failed ({status}): {body}"
        )));
    }

    let installation: InstallationResponse = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse installation: {e}")))?;

    Ok(installation.account.login)
}

// ── Repository listing ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RepoItem {
    pub full_name: String,
}

#[derive(Deserialize)]
struct ReposResponse {
    pub repositories: Vec<RepoItem>,
}

/// Lists all repositories accessible to the installation.
#[instrument(skip(installation_token))]
pub async fn list_installation_repos(installation_token: &str) -> Result<Vec<String>, ApiError> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/installation/repositories?per_page=100")
        .bearer_auth(installation_token)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub list-repos request failed ({status}): {body}"
        )));
    }

    let repos_resp: ReposResponse = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse repos response: {e}")))?;

    Ok(repos_resp
        .repositories
        .into_iter()
        .map(|r| r.full_name)
        .collect())
}

// ── OAuth token exchange ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct OAuthTokenResponse {
    pub access_token: String,
}

#[derive(Deserialize)]
struct GithubUser {
    pub id: i64,
    pub login: String,
}

/// Exchanges an OAuth authorization code for a user access token.
/// Returns `(access_token, github_user_id, github_login)`.
#[instrument(skip(github, code))]
pub async fn exchange_oauth_code(
    github: &GithubConfig,
    code: &str,
) -> Result<(String, i64, String), ApiError> {
    let client = reqwest::Client::new();

    // Exchange code for token
    let token_resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "Weld/1.0")
        .form(&[
            ("client_id", github.oauth_client_id.as_str()),
            ("client_secret", github.oauth_client_secret.as_str()),
            ("code", code),
        ])
        .send()
        .await?;

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub OAuth token exchange failed ({status}): {body}"
        )));
    }

    let token_data: OAuthTokenResponse = token_resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse OAuth token response: {e}")))?;

    // Fetch authenticated user info
    let user_resp = client
        .get("https://api.github.com/user")
        .bearer_auth(&token_data.access_token)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Weld/1.0")
        .send()
        .await?;

    if !user_resp.status().is_success() {
        let status = user_resp.status();
        let body = user_resp.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "GitHub get-user request failed ({status}): {body}"
        )));
    }

    let user: GithubUser = user_resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse GitHub user: {e}")))?;

    Ok((token_data.access_token, user.id, user.login))
}

// ── Encryption ─────────────────────────────────────────────────────────────

/// Derives a 32-byte AES key from any-length `master_key` string via SHA-256.
fn derive_aes_key(master_key: &str) -> [u8; 32] {
    sha2::Sha256::digest(master_key.as_bytes()).into()
}

/// AES-256-GCM encrypts `plaintext`. Returns `(ciphertext, nonce)`.
pub fn encrypt_bytes(master_key: &str, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), ApiError> {
    let key_bytes = derive_aes_key(master_key);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut rand::thread_rng());
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| ApiError::Internal("Token encryption failed".into()))?;
    Ok((ciphertext, nonce.to_vec()))
}

// ── State tokens (CSRF) ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct GithubStateToken {
    pub sub: String,  // UUID as string (team_id or user_id depending on kind)
    pub kind: String, // "install" or "oauth"
    exp: i64,
}

const STATE_TOKEN_TTL_SECS: i64 = 900; // 15 minutes

/// Signs a short-lived state token used for CSRF protection in GitHub callbacks.
pub fn sign_state_token(jwt_secret: &str, id: Uuid, kind: &str) -> Result<String, ApiError> {
    let claims = GithubStateToken {
        sub: id.to_string(),
        kind: kind.to_string(),
        exp: Utc::now().timestamp() + STATE_TOKEN_TTL_SECS,
    };
    encode(
        &Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(jwt_secret.as_ref()),
    )
    .map_err(|e| ApiError::Internal(format!("Failed to sign state token: {e}")))
}

/// Verifies and decodes a state token. Checks expiry and `kind`.
pub fn verify_state_token(
    jwt_secret: &str,
    token: &str,
    expected_kind: &str,
) -> Result<Uuid, ApiError> {
    let mut validation = jsonwebtoken::Validation::default();
    validation.validate_exp = false; // we check exp manually below
    validation.required_spec_claims.clear();

    let data = jsonwebtoken::decode::<GithubStateToken>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_ref()),
        &validation,
    )
    .map_err(|_| ApiError::Validation("Invalid or tampered state token".into()))?;

    if data.claims.exp < Utc::now().timestamp() {
        return Err(ApiError::Validation("State token has expired".into()));
    }

    if data.claims.kind != expected_kind {
        return Err(ApiError::Validation("State token kind mismatch".into()));
    }

    Uuid::parse_str(&data.claims.sub)
        .map_err(|_| ApiError::Validation("State token contains invalid UUID".into()))
}

// ── Config guard ───────────────────────────────────────────────────────────

/// Returns a reference to the GitHub config or a 501 error if not configured.
pub fn require_github_config(
    github: &Option<std::sync::Arc<GithubConfig>>,
) -> Result<&GithubConfig, ApiError> {
    github
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GitHub integration is not configured".into()))
}

/// Returns the GitHub OAuth URL for user identity linking.
pub fn build_oauth_url(github: &GithubConfig, state_token: &str) -> String {
    format!(
        "https://github.com/login/oauth/authorize?client_id={}&state={state_token}&scope=read:user",
        github.oauth_client_id
    )
}

/// Returns the GitHub App install URL for a team.
pub fn build_install_url(github: &GithubConfig, state_token: &str) -> String {
    format!(
        "https://github.com/apps/{}/installations/new?state={state_token}",
        github.app_slug
    )
}
