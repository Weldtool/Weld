use crate::errors::ApiError;
use crate::middleware::auth::Claims;
use axum::http::HeaderMap;
use chrono::Utc;
use sha2::Digest;
use uuid::Uuid;

pub fn hash_refresh_token(token: &str) -> String {
    format!("{:x}", sha2::Sha256::digest(token.as_bytes()))
}

/// Build a JWT access token with a 15-minute expiry.
pub fn generate_access_token(user_id: Uuid, role: &str, secret: &str) -> Result<String, ApiError> {
    let claims = Claims {
        sub: user_id.to_string(),
        role: role.to_string(),
        iat: Utc::now().timestamp(),
        exp: (Utc::now() + chrono::Duration::minutes(15)).timestamp(),
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_ref()),
    )
    .map_err(|e| ApiError::Internal(format!("Token generation failed: {e}")))
}

/// Generate a new refresh token, store its hash in the DB, return the raw token.
pub async fn create_refresh_token(pool: &sqlx::PgPool, user_id: Uuid) -> Result<String, ApiError> {
    let raw_token = Uuid::new_v4().to_string();
    let hashed = hash_refresh_token(&raw_token);
    sqlx::query!(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
        VALUES ($1, $2, $3)
        "#,
        user_id,
        hashed,
        Utc::now() + chrono::Duration::days(7)
    )
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?;
    Ok(raw_token)
}

/// Extract the refresh_token value from the Cookie header.
/// Returns Unauthorized (not Internal) if the cookie is missing.
pub fn extract_refresh_token(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
                if parts.len() == 2 && parts[0] == "refresh_token" {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| ApiError::Unauthorized("Missing refresh token cookie".into()))
}

/// Build the Set-Cookie string for a refresh token.
/// When `secure` is true we use SameSite=None so the cookie is sent on
/// cross-site requests (frontend and backend on different Railway subdomains).
/// SameSite=None requires Secure, so browsers reject it otherwise — in dev
/// (HTTP, secure=false) we fall back to Lax which works for same-origin.
pub fn build_refresh_token_cookie(token: &str, secure: bool) -> String {
    refresh_token_cookie(token, 604_800, secure)
}

/// Build the Set-Cookie string that removes the refresh token (logout). The
/// attributes must match the ones it was set with or the browser keeps it.
pub fn build_cleared_refresh_token_cookie(secure: bool) -> String {
    refresh_token_cookie("", 0, secure)
}

fn refresh_token_cookie(token: &str, max_age_secs: u32, secure: bool) -> String {
    let (same_site, secure_flag) = if secure {
        ("None", "; Secure")
    } else {
        ("Lax", "")
    };
    format!(
        "refresh_token={}; HttpOnly; SameSite={}; Path=/api/v1/auth; Max-Age={}{}",
        token, same_site, max_age_secs, secure_flag
    )
}
