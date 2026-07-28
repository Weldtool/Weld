use aes_gcm::{
    Aes256Gcm, Key,
    aead::{Aead, AeadCore, KeyInit},
};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tracing::instrument;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::state::GithubConfig;

// ── Shared HTTP client ─────────────────────────────────────────────────────

/// One client for every GitHub call. Building a `reqwest::Client` per request
/// throws away the connection pool, so each call paid for a fresh TCP + TLS
/// handshake.
static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Weld/1.0")
        .timeout(Duration::from_secs(20))
        .build()
        .expect("reqwest client builds from static config")
});

/// The shared GitHub HTTP client. Sets `User-Agent` already; callers only need
/// to add auth and `Accept`.
pub fn gh_client() -> &'static reqwest::Client {
    &HTTP
}

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

/// GitHub installation tokens are valid for 60 minutes. Cache them for 50 so
/// there is headroom for clock skew and in-flight requests.
const TOKEN_TTL: Duration = Duration::from_secs(50 * 60);

// ponytail: process-global cache, which is right for a single server process.
// Move it into AppState if a deploy ever needs to evict a tenant's token early.
static TOKEN_CACHE: LazyLock<Mutex<HashMap<i64, (String, Instant)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns a valid installation access token for `installation_id`, minting a
/// new one only when the cache is empty or stale.
///
/// Before this cached, every single GitHub call — every issue list, every card
/// chip refresh — spent an RSA signature and a full extra API round-trip
/// re-minting a token that was already good for another 59 minutes.
#[instrument(skip(github))]
pub async fn create_installation_token(
    github: &GithubConfig,
    installation_id: i64,
) -> Result<String, ApiError> {
    if let Some((token, minted_at)) = cache_lookup(installation_id)
        && minted_at.elapsed() < TOKEN_TTL
    {
        return Ok(token);
    }

    let jwt = create_app_jwt(github)?;
    let resp = gh_client()
        .post(format!(
            "https://api.github.com/app/installations/{installation_id}/access_tokens"
        ))
        .bearer_auth(&jwt)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("installation token request", resp).await);
    }

    let token_resp: InstallationTokenResponse = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse installation token: {e}")))?;

    cache_store(installation_id, &token_resp.token);
    Ok(token_resp.token)
}

fn cache_lookup(installation_id: i64) -> Option<(String, Instant)> {
    // A poisoned mutex means a previous holder panicked mid-update; the cache
    // is just an optimisation, so recover rather than propagate.
    let guard = TOKEN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    guard.get(&installation_id).cloned()
}

fn cache_store(installation_id: i64, token: &str) {
    let mut guard = TOKEN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(installation_id, (token.to_string(), Instant::now()));
}

// ── Error mapping ──────────────────────────────────────────────────────────

/// Converts a failed GitHub response into the right `ApiError`.
///
/// Most GitHub 4xx responses describe something the *caller* got wrong — a
/// branch that does not exist, a repo the App cannot see, a rate limit. Mapping
/// them all to `Internal` turned every user mistake into a 500 and an ERROR log.
pub async fn github_error(context: &str, resp: reqwest::Response) -> ApiError {
    let status = resp.status();
    let rate_limited = resp
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "0");
    let text = resp.text().await.unwrap_or_default();
    let detail = parse_github_message(&text).unwrap_or_else(|| text.clone());

    match status.as_u16() {
        // Our own credentials are bad — the user cannot fix this.
        401 => ApiError::Internal(format!("GitHub rejected our credentials ({context})")),
        403 | 429 if rate_limited => {
            ApiError::Conflict("GitHub API rate limit reached. Try again in a few minutes.".into())
        }
        403 => ApiError::Forbidden(format!(
            "GitHub denied this request ({context}): {detail}. \
             Check the Weld App has access to this repository."
        )),
        404 => ApiError::NotFound(format!("Not found on GitHub ({context}): {detail}")),
        // 405 = not mergeable (failing checks, conflicts, branch protection);
        // 409 = the branch moved since the client last looked.
        405 | 409 => ApiError::Conflict(format!("GitHub could not complete this: {detail}")),
        422 => ApiError::Validation(format!("GitHub rejected this request: {detail}")),
        _ => ApiError::Internal(format!("GitHub {context} failed ({status}): {text}")),
    }
}

/// Pulls the human-readable message out of a GitHub error response body.
pub fn parse_github_message(text: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    if let Some(errors) = json["errors"].as_array() {
        let joined: Vec<String> = errors
            .iter()
            .filter_map(|e| e["message"].as_str().map(str::to_string))
            .collect();
        if !joined.is_empty() {
            return Some(joined.join("; "));
        }
    }
    json["message"].as_str().map(str::to_string)
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
    let resp = gh_client()
        .get(format!(
            "https://api.github.com/app/installations/{installation_id}"
        ))
        .bearer_auth(&jwt)
        .header("Accept", "application/vnd.github.v3+json")
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
    let resp = gh_client()
        .get("https://api.github.com/installation/repositories?per_page=100")
        .bearer_auth(installation_token)
        .header("Accept", "application/vnd.github.v3+json")
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
    /// Present when the App has "Expire user authorization tokens" enabled.
    /// Absent means a non-expiring token, which we simply never refresh.
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub refresh_token_expires_in: Option<i64>,
}

/// A user-to-server token as GitHub issued it.
pub struct UserToken {
    pub access_token: String,
    pub expires_at: Option<chrono::DateTime<Utc>>,
    pub refresh_token: Option<String>,
    pub refresh_expires_at: Option<chrono::DateTime<Utc>>,
}

impl From<OAuthTokenResponse> for UserToken {
    fn from(r: OAuthTokenResponse) -> Self {
        let now = Utc::now();
        Self {
            access_token: r.access_token,
            expires_at: r.expires_in.map(|s| now + chrono::Duration::seconds(s)),
            refresh_token: r.refresh_token,
            refresh_expires_at: r
                .refresh_token_expires_in
                .map(|s| now + chrono::Duration::seconds(s)),
        }
    }
}

#[derive(Deserialize)]
struct GithubUser {
    pub id: i64,
    pub login: String,
}

/// Exchanges an authorization code for a user-to-server token.
/// Returns `(token, github_user_id, github_login)`.
#[instrument(skip(github, code))]
pub async fn exchange_oauth_code(
    github: &GithubConfig,
    code: &str,
) -> Result<(UserToken, i64, String), ApiError> {
    let client = gh_client();

    // Exchange code for token
    let token_resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
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
    let token = UserToken::from(token_data);

    // Fetch authenticated user info
    let user_resp = client
        .get("https://api.github.com/user")
        .bearer_auth(&token.access_token)
        .header("Accept", "application/vnd.github.v3+json")
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

    Ok((token, user.id, user.login))
}

/// Trades a refresh token for a fresh user-to-server token.
///
/// User tokens live 8 hours, so anything acting on a user's behalf has to be
/// able to do this silently rather than making them re-authorise twice a day.
#[instrument(skip(github, refresh_token))]
pub async fn refresh_user_token(
    github: &GithubConfig,
    refresh_token: &str,
) -> Result<UserToken, ApiError> {
    let resp = gh_client()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", github.oauth_client_id.as_str()),
            ("client_secret", github.oauth_client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(github_error("refresh user token", resp).await);
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse refresh response: {e}")))?;

    // GitHub reports refresh failures as 200 + {"error": "..."}, so the status
    // check above is not enough on its own.
    if let Some(err) = body["error"].as_str() {
        return Err(ApiError::Unauthorized(format!(
            "GitHub refused to refresh the token: {err}"
        )));
    }

    let parsed: OAuthTokenResponse = serde_json::from_value(body)
        .map_err(|e| ApiError::Internal(format!("Unexpected refresh response: {e}")))?;
    Ok(UserToken::from(parsed))
}

// ── Encryption ─────────────────────────────────────────────────────────────

/// Derives a 32-byte AES key from any-length `master_key` string via SHA-256.
fn derive_aes_key(master_key: &str) -> [u8; 32] {
    sha2::Sha256::digest(master_key.as_bytes()).into()
}

/// AES-256-GCM decrypts `ciphertext` using `nonce`.
pub fn decrypt_bytes(
    master_key: &str,
    ciphertext: &[u8],
    nonce: &[u8],
) -> Result<Vec<u8>, ApiError> {
    let key_bytes = derive_aes_key(master_key);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = aes_gcm::Nonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| ApiError::Internal("Token decryption failed".into()))
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

/// Returns a reference to the GitHub config, or a 400 if the server has no
/// GitHub App configured.
///
/// This is a deployment state, not a crash: it used to return `Internal`, so an
/// unconfigured server logged an ERROR and told the UI "server error" instead of
/// "GitHub isn't set up". Matches what the task routes already returned.
pub fn require_github_config(
    github: &Option<std::sync::Arc<GithubConfig>>,
) -> Result<&GithubConfig, ApiError> {
    github.as_deref().ok_or_else(|| {
        ApiError::Validation("GitHub integration is not configured on the server".into())
    })
}

/// Returns the authorize URL for linking a user's GitHub identity.
///
/// No `scope` parameter: these are GitHub App user-to-server tokens, whose
/// permissions are exactly the App's permissions on the repos where it is
/// installed, intersected with what the user themselves can reach. That is the
/// whole reason to prefer them over OAuth App tokens — there is no scope to
/// over-request, and the token can never touch a repo the App is not on.
pub fn build_oauth_url(github: &GithubConfig, state_token: &str) -> String {
    format!(
        "https://github.com/login/oauth/authorize?client_id={}&state={state_token}",
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
