use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::github::{
    self, GithubConnectionRecord,
    helpers::{
        build_install_url, build_oauth_url, require_github_config, sign_state_token,
        verify_state_token,
    },
};
use crate::services::team::helpers::require_team_role;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

// ── Shared response type ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GithubConnectionResponse {
    pub id: Uuid,
    pub team_id: Uuid,
    pub installation_id: i64,
    pub account_login: String,
    pub repos: Vec<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<GithubConnectionRecord> for GithubConnectionResponse {
    fn from(r: GithubConnectionRecord) -> Self {
        Self {
            id: r.id,
            team_id: r.team_id,
            installation_id: r.installation_id,
            account_login: r.account_login,
            repos: r.repos,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// ── GET /api/v1/teams/:team_id/github/install-url ─────────────────────────

#[derive(Debug, Serialize)]
pub struct InstallUrlResponse {
    pub url: String,
}

/// Returns the GitHub App installation URL for a team. Requires Admin+.
pub async fn get_install_url(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<Json<InstallUrlResponse>, ApiError> {
    require_team_role(&state.db, team_id, auth.user_id, TeamRole::Admin).await?;
    let gh_cfg = require_github_config(&state.github)?;
    let state_token = sign_state_token(&state.jwt_secret, team_id, "install")?;
    let url = build_install_url(gh_cfg, &state_token);
    Ok(Json(InstallUrlResponse { url }))
}

// ── GET /api/v1/github/setup (post-install callback from GitHub) ──────────

#[derive(Debug, Deserialize)]
pub struct SetupCallbackQuery {
    pub installation_id: i64,
    pub setup_action: Option<String>,
    pub state: String,
}

/// GitHub redirects here after a user installs the Weld GitHub App.
/// Validates the CSRF state token, calls the GitHub API, and stores the
/// connection. Redirects the browser back to the frontend on success.
pub async fn setup_callback(
    State(state): State<AppState>,
    Query(params): Query<SetupCallbackQuery>,
) -> Result<Response, ApiError> {
    // Validate the CSRF state token before checking GitHub config so invalid
    // tokens always return 400, even when the integration is not configured.
    let team_id = verify_state_token(&state.jwt_secret, &params.state, "install")?;

    let gh_cfg = require_github_config(&state.github)?;

    // Reject unsupported setup actions (e.g. "delete")
    if let Some(ref action) = params.setup_action
        && action != "install"
        && action != "update"
    {
        let redirect_url = format!("{}?github_error=unsupported_action", state.frontend_url);
        return Ok(Redirect::to(&redirect_url).into_response());
    }

    // caller_id is derived from the team's created_by — use a sentinel for the
    // post-install flow since the request has no auth header. We look up the
    // team owner to satisfy the RBAC check inside connect_github.
    let team_owner_id = sqlx::query_scalar!(
        "SELECT owner_id FROM teams WHERE id = $1 AND deleted_at IS NULL",
        team_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Team not found".into()))?;

    let conn = github::connect_github(
        &state.db,
        gh_cfg,
        team_id,
        team_owner_id,
        params.installation_id,
    )
    .await?;

    tracing::info!(
        team_id = %team_id,
        account_login = %conn.account_login,
        "GitHub App installed"
    );

    let redirect_url = format!(
        "{}/teams/{team_id}?github_connected=true",
        state.frontend_url
    );
    Ok(Redirect::to(&redirect_url).into_response())
}

// ── GET /api/v1/github/oauth/url ──────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OAuthUrlResponse {
    pub url: String,
}

/// Returns the GitHub OAuth URL so the browser can link the user's GitHub
/// identity to their Weld account.
pub async fn get_oauth_url(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<OAuthUrlResponse>, ApiError> {
    let gh_cfg = require_github_config(&state.github)?;
    let state_token = sign_state_token(&state.jwt_secret, auth.user_id, "oauth")?;
    let url = build_oauth_url(gh_cfg, &state_token);
    Ok(Json(OAuthUrlResponse { url }))
}

// ── GET /api/v1/github/oauth/callback ─────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: String,
}

/// GitHub redirects here after the user authorises the Weld OAuth App.
/// Validates the CSRF state token, exchanges the code, stores the encrypted
/// token, and redirects back to the frontend.
pub async fn oauth_callback(
    State(state): State<AppState>,
    Query(params): Query<OAuthCallbackQuery>,
) -> Result<Response, ApiError> {
    // Validate CSRF state before checking GitHub config so invalid tokens
    // always return 400, even when the integration is not configured.
    let weld_user_id = verify_state_token(&state.jwt_secret, &params.state, "oauth")?;

    let gh_cfg = require_github_config(&state.github)?;

    let master_key = state
        .master_encryption_key
        .as_deref()
        .ok_or_else(|| ApiError::Internal("MASTER_ENCRYPTION_KEY is not configured".into()))?;

    let linked =
        github::handle_oauth_callback(&state.db, gh_cfg, master_key, weld_user_id, &params.code)
            .await?;

    tracing::info!(
        user_id = %weld_user_id,
        github_login = %linked.github_login,
        "GitHub user linked"
    );

    let redirect_url = format!("{}?github_linked=true", state.frontend_url);
    Ok(Redirect::to(&redirect_url).into_response())
}

// ── DELETE /api/v1/teams/:team_id/github ──────────────────────────────────

pub async fn disconnect(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    github::disconnect_github(&state.db, team_id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /api/v1/teams/:team_id/github ─────────────────────────────────────

pub async fn get_connection(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<Json<GithubConnectionResponse>, ApiError> {
    let conn = github::get_github_connection(&state.db, team_id, auth.user_id).await?;
    Ok(Json(conn.into()))
}

// ── PATCH /api/v1/teams/:team_id/github ───────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpdateReposRequest {
    pub repos: Vec<String>,
}

pub async fn update_repos(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    Json(input): Json<UpdateReposRequest>,
) -> Result<Json<GithubConnectionResponse>, ApiError> {
    let conn = github::update_github_repos(&state.db, team_id, auth.user_id, input.repos).await?;
    Ok(Json(conn.into()))
}

// ── POST /api/v1/github/webhook (stub) ────────────────────────────────────

pub async fn webhook_stub(State(_state): State<AppState>) -> impl IntoResponse {
    // Signature verification and event processing are implemented in issue #28.
    StatusCode::OK
}
