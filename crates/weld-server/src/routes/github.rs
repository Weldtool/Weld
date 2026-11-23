use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::github::{
    self, GithubConnectionRecord,
    helpers::{
        VerifiedState, build_install_url, build_oauth_url, cleared_state_cookie, issue_bound_state,
        require_github_config, user_can_access_installation, verify_bound_state,
    },
    user_token::user_access_token,
};
use crate::services::study::github_fixtures;
use crate::services::team::helpers::require_team_role;
use crate::state::AppState;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
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
///
/// Also sets the cookie that ties the returned state token to this browser, so
/// the setup callback can tell it is completing a flow this user started.
pub async fn get_install_url(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<(HeaderMap, Json<InstallUrlResponse>), ApiError> {
    require_team_role(&state.db, team_id, auth.user_id, TeamRole::Admin).await?;
    let gh_cfg = require_github_config(&state.github)?;
    let bound = issue_bound_state(
        &state.jwt_secret,
        "install",
        team_id,
        auth.user_id,
        state.secure_cookies,
    )?;
    let url = build_install_url(gh_cfg, &bound.token);
    Ok((
        state_cookie_headers(&bound.cookie)?,
        Json(InstallUrlResponse { url }),
    ))
}

fn state_cookie_headers(cookie: &str) -> Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("Failed to set cookie header".into()))?,
    );
    Ok(headers)
}

/// Sends the browser back to the settings page with a machine-readable reason.
fn settings_error_redirect(state: &AppState, code: &str) -> Response {
    Redirect::to(&format!(
        "{}/settings?github_error={code}",
        state.frontend_url
    ))
    .into_response()
}

// ── GET /api/v1/github/setup (post-install callback from GitHub) ──────────

#[derive(Debug, Deserialize)]
pub struct SetupCallbackQuery {
    pub installation_id: i64,
    pub setup_action: Option<String>,
    pub state: String,
}

/// GitHub redirects here after a user installs the Weld GitHub App.
///
/// The redirect carries no auth header, and its `installation_id` is an
/// unauthenticated query parameter, so two things are proven before anything is
/// stored: that this browser started the flow (bound state token), and that the
/// installer's linked GitHub account can actually reach that installation.
///
/// Requires the App's "Request user authorization (OAuth) during installation"
/// to be off: with it on, GitHub skips this Setup URL entirely.
///
/// Every outcome is a redirect back to Settings: the browser arrived from
/// GitHub, and a JSON error body would leave the user on a dead end.
pub async fn setup_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SetupCallbackQuery>,
) -> Response {
    let verified = match verify_bound_state(&state.jwt_secret, &params.state, "install", &headers) {
        Ok(verified) => verified,
        Err(err) => return state_failure(&state, "install", err),
    };

    match complete_setup(&state, verified, &params).await {
        Ok(()) => success_redirect(&state, "install", "github_connected=true"),
        Err(err) => callback_failure(&state, "install", err),
    }
}

/// Outcome of a setup callback whose state has already been verified.
async fn complete_setup(
    state: &AppState,
    verified: VerifiedState,
    params: &SetupCallbackQuery,
) -> Result<(), CallbackError> {
    let team_id = verified.subject;
    let gh_cfg = require_github_config(&state.github)?;

    // Reject unsupported setup actions (e.g. "delete")
    if let Some(ref action) = params.setup_action
        && action != "install"
        && action != "update"
    {
        return Err(CallbackError::Code("unsupported_action"));
    }

    // The state token was minted for an Admin+, but up to 15 minutes ago —
    // re-check so a demotion in between is honoured.
    require_team_role(&state.db, team_id, verified.actor, TeamRole::Admin).await?;

    // The installation is checked against the GitHub account this Weld user
    // has linked; without one there is nothing to check it against.
    let user_token = match state.master_encryption_key.as_deref() {
        Some(key) => user_access_token(&state.db, gh_cfg, key, verified.actor).await?,
        None => None,
    };
    let Some(user_token) = user_token else {
        return Err(CallbackError::Code("link_required"));
    };

    if !user_can_access_installation(&user_token, params.installation_id).await? {
        tracing::warn!(
            team_id = %team_id,
            user_id = %verified.actor,
            installation_id = params.installation_id,
            "Rejected GitHub setup: user cannot access that installation"
        );
        return Err(CallbackError::Code("installation_not_authorized"));
    }

    let conn = github::connect_github(
        &state.db,
        gh_cfg,
        team_id,
        verified.actor,
        params.installation_id,
    )
    .await?;

    tracing::info!(
        team_id = %team_id,
        account_login = %conn.account_login,
        "GitHub App installed"
    );
    Ok(())
}

// ── Callback outcomes ─────────────────────────────────────────────────────

/// Why a callback whose state verified did not complete.
enum CallbackError {
    /// A known reason, sent as a `github_error` code the settings page has a
    /// message for.
    Code(&'static str),
    Api(ApiError),
}

impl From<ApiError> for CallbackError {
    fn from(err: ApiError) -> Self {
        Self::Api(err)
    }
}

/// Redirects to Settings with `query`, clearing the flow's state cookie so the
/// same state can't be replayed.
fn success_redirect(state: &AppState, kind: &str, query: &str) -> Response {
    (
        [(SET_COOKIE, cleared_state_cookie(kind, state.secure_cookies))],
        Redirect::to(&format!("{}/settings?{query}", state.frontend_url)),
    )
        .into_response()
}

/// The state didn't verify: expired, tampered, or started in another browser
/// (usually a lost or blocked cookie).
fn state_failure(state: &AppState, kind: &str, err: ApiError) -> Response {
    tracing::warn!(flow = kind, error = %err, "GitHub callback state rejected");
    settings_error_redirect(state, "state_mismatch")
}

/// Any other failure after the state verified.
fn callback_failure(state: &AppState, kind: &str, err: CallbackError) -> Response {
    let err = match err {
        CallbackError::Code(code) => return settings_error_redirect(state, code),
        CallbackError::Api(err) => err,
    };
    let code = match &err {
        ApiError::Forbidden(_) | ApiError::NotFound(_) => "forbidden",
        _ => "failed",
    };
    match &err {
        ApiError::Internal(_) | ApiError::Unavailable(_) => {
            tracing::error!(flow = kind, error = %err, "GitHub callback failed")
        }
        _ => tracing::warn!(flow = kind, error = %err, "GitHub callback failed"),
    }
    settings_error_redirect(state, code)
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
) -> Result<(HeaderMap, Json<OAuthUrlResponse>), ApiError> {
    let gh_cfg = require_github_config(&state.github)?;
    let bound = issue_bound_state(
        &state.jwt_secret,
        "oauth",
        auth.user_id,
        auth.user_id,
        state.secure_cookies,
    )?;
    let url = build_oauth_url(gh_cfg, &bound.token);
    Ok((
        state_cookie_headers(&bound.cookie)?,
        Json(OAuthUrlResponse { url }),
    ))
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
///
/// The state must be bound to this browser: otherwise an attacker could send a
/// victim an authorise link carrying the attacker's state, and the victim's
/// GitHub token would be stored on the attacker's Weld account.
pub async fn oauth_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<OAuthCallbackQuery>,
) -> Response {
    let weld_user_id = match verify_bound_state(&state.jwt_secret, &params.state, "oauth", &headers)
    {
        Ok(verified) => verified.subject,
        Err(err) => return state_failure(&state, "oauth", err),
    };

    match complete_oauth(&state, weld_user_id, &params.code).await {
        Ok(()) => success_redirect(&state, "oauth", "github_linked=true"),
        Err(err) => callback_failure(&state, "oauth", err),
    }
}

async fn complete_oauth(
    state: &AppState,
    weld_user_id: Uuid,
    code: &str,
) -> Result<(), CallbackError> {
    let gh_cfg = require_github_config(&state.github)?;

    let master_key = state
        .master_encryption_key
        .as_deref()
        .ok_or_else(|| ApiError::Internal("MASTER_ENCRYPTION_KEY is not configured".into()))?;

    let linked =
        github::handle_oauth_callback(&state.db, gh_cfg, master_key, weld_user_id, code).await?;

    tracing::info!(
        user_id = %weld_user_id,
        github_login = %linked.github_login,
        "GitHub user linked"
    );
    Ok(())
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

// ── GET /api/v1/teams/:team_id/github/repos ───────────────────────────────

#[derive(Debug, Serialize)]
pub struct InstallationReposResponse {
    pub repos: Vec<String>,
}

/// Lists the repositories accessible via the team's GitHub App installation.
/// Live-fetched from GitHub on every call.
pub async fn list_installation_repos(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<Json<InstallationReposResponse>, ApiError> {
    let gh_cfg = require_github_config(&state.github)?;
    let repos =
        github::list_team_installation_repos(&state.db, gh_cfg, team_id, auth.user_id).await?;
    Ok(Json(InstallationReposResponse { repos }))
}

// ── POST /api/v1/github/webhook ───────────────────────────────────────────

/// Receives GitHub webhook deliveries.
///
/// The raw body is kept intact for HMAC-SHA256 signature verification before
/// any deserialization occurs.  Idempotency is enforced via the
/// `X-GitHub-Delivery` header tracked in `github_webhook_deliveries`.
pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let gh_cfg = require_github_config(&state.github)?;

    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing X-Hub-Signature-256".into()))?;

    let event = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let delivery_id = headers
        .get("x-github-delivery")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Validation("Missing X-GitHub-Delivery".into()))?
        .to_string();

    github::verify_signature(&gh_cfg.webhook_secret, &body, signature)?;

    github::handle_webhook(&state.db, &state.hub, &event, &delivery_id, &body).await?;
    Ok(StatusCode::OK)
}

// ── GET /api/v1/projects/:id/github/issues ────────────────────────────────

/// Optional `?state=open|closed|all` filter shared by the issue and PR lists.
#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    pub state: Option<String>,
}

pub async fn list_project_issues(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    if state.study_mode {
        return Ok(Json(
            github_fixtures::list_issues(&state.db, project_id, auth.user_id).await?,
        ));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let issues = github::sync::list_project_issues(
        &state.db,
        gh_cfg,
        project_id,
        auth.user_id,
        q.state.as_deref(),
    )
    .await?;
    Ok(Json(issues))
}

// ── GET /api/v1/projects/:id/github/issues/:number ────────────────────────

pub async fn get_project_issue(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((project_id, issue_number)): Path<(Uuid, i64)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if state.study_mode {
        return Ok(Json(
            github_fixtures::get_issue(&state.db, project_id, issue_number, auth.user_id).await?,
        ));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let issue =
        github::sync::get_project_issue(&state.db, gh_cfg, project_id, issue_number, auth.user_id)
            .await?;
    Ok(Json(issue))
}

// ── GET /api/v1/projects/:id/github/prs ───────────────────────────────────

pub async fn list_project_prs(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    if state.study_mode {
        return Ok(Json(
            github_fixtures::list_prs(&state.db, project_id, auth.user_id).await?,
        ));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let prs = github::sync::list_project_prs(
        &state.db,
        gh_cfg,
        project_id,
        auth.user_id,
        q.state.as_deref(),
    )
    .await?;
    Ok(Json(prs))
}

// ── GET /api/v1/projects/:id/github/prs/:number ───────────────────────────

/// A single PR plus its changed files, which is what the review view needs.
pub async fn get_project_pr(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if state.study_mode {
        // Study fixtures already carry `files` on the list entries.
        let prs = github_fixtures::list_prs(&state.db, project_id, auth.user_id).await?;
        return prs
            .into_iter()
            .find(|pr| pr["number"].as_i64() == Some(number))
            .map(Json)
            .ok_or_else(|| ApiError::NotFound("Pull request not found".into()));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let pr =
        github::sync::get_project_pr(&state.db, gh_cfg, project_id, number, auth.user_id).await?;
    Ok(Json(pr))
}

// ── POST /api/v1/projects/:id/github/prs/:number/reviews ──────────────────

#[derive(Debug, Deserialize)]
pub struct SubmitReviewRequest {
    pub verdict: github::sync::ReviewVerdict,
    #[serde(default)]
    pub body: String,
}

pub async fn submit_pr_review(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(input): Json<SubmitReviewRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if state.study_mode {
        return Err(ApiError::Forbidden(
            "GitHub writes are disabled in study mode".into(),
        ));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let review = github::sync::submit_pr_review(
        &state.db,
        gh_cfg,
        state.master_encryption_key.as_deref(),
        project_id,
        number,
        auth.user_id,
        input.verdict,
        &input.body,
    )
    .await?;
    Ok(Json(review))
}

// ── PUT /api/v1/projects/:id/github/prs/:number/merge ─────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct MergePrRequest {
    #[serde(default)]
    pub method: github::sync::MergeMethod,
}

pub async fn merge_pr(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    body: Option<Json<MergePrRequest>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if state.study_mode {
        return Err(ApiError::Forbidden(
            "GitHub writes are disabled in study mode".into(),
        ));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let method = body.map(|Json(b)| b.method).unwrap_or_default();
    let result = github::sync::merge_pr(
        &state.db,
        gh_cfg,
        state.master_encryption_key.as_deref(),
        project_id,
        number,
        auth.user_id,
        method,
    )
    .await?;
    Ok(Json(result))
}

// ── GET /api/v1/projects/:id/github/branches ──────────────────────────────

pub async fn list_project_branches(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<github::sync::RepoBranches>, ApiError> {
    if state.study_mode {
        return Ok(Json(
            github_fixtures::list_branches(&state.db, project_id, auth.user_id).await?,
        ));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let branches = github::sync::list_branches(&state.db, gh_cfg, project_id, auth.user_id).await?;
    Ok(Json(branches))
}

// ── GET /api/v1/projects/:id/github/pr-template ───────────────────────────

#[derive(Debug, Serialize)]
pub struct PrTemplateResponse {
    pub template: Option<String>,
}

pub async fn get_pr_template(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<PrTemplateResponse>, ApiError> {
    if state.study_mode {
        let template =
            github_fixtures::get_pr_template(&state.db, project_id, auth.user_id).await?;
        return Ok(Json(PrTemplateResponse { template }));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let template =
        github::sync::get_pr_template(&state.db, gh_cfg, project_id, auth.user_id).await?;
    Ok(Json(PrTemplateResponse { template }))
}

// ── GET /api/v1/projects/:id/github/issue-templates ───────────────────────

pub async fn list_issue_templates(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<github::sync::IssueTemplate>>, ApiError> {
    if state.study_mode {
        return Ok(Json(
            github_fixtures::list_issue_templates(&state.db, project_id, auth.user_id).await?,
        ));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let templates =
        github::sync::list_issue_templates(&state.db, gh_cfg, project_id, auth.user_id).await?;
    Ok(Json(templates))
}

// ── POST /api/v1/projects/:id/github/labels/sync ──────────────────────────

pub async fn sync_project_labels(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<weld_core::models::task::Label>>, ApiError> {
    if state.study_mode {
        return Err(ApiError::Forbidden(
            "GitHub label sync is disabled in study mode".into(),
        ));
    }
    let gh_cfg = require_github_config(&state.github)?;
    let labels =
        github::sync::sync_labels_from_github(&state.db, gh_cfg, project_id, auth.user_id).await?;
    Ok(Json(labels))
}
