use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::study;
use crate::services::study::telemetry::EventInput;
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ProvisionResponse {
    pub participant_code: String,
    pub agent_condition: String,
    pub team_id: Uuid,
    pub project_id: Uuid,
    pub access_token: String,
    pub token_type: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ProvisionRequest {
    /// Version of the consent text the participant affirmed before provisioning.
    pub consent_version: Option<String>,
}

/// Provision a fresh study participant with an isolated PetTracker clone and a
/// session. Public (no auth — the participant has no account yet) but only
/// available under STUDY_MODE; otherwise it 404s so it's invisible in the
/// normal product.
pub async fn provision(
    State(state): State<AppState>,
    body: Option<Json<ProvisionRequest>>,
) -> Result<(StatusCode, HeaderMap, Json<ProvisionResponse>), ApiError> {
    if !state.study_mode {
        return Err(ApiError::NotFound("Not found".into()));
    }

    let consent_version = body.and_then(|Json(b)| b.consent_version);
    let result =
        study::provision_participant(&state.db, &state.jwt_secret, consent_version.as_deref())
            .await?;

    let mut headers = HeaderMap::new();
    let cookie = crate::services::auth::helpers::build_refresh_token_cookie(
        &result.refresh_token,
        state.secure_cookies,
    );
    headers.insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("Failed to set cookie header".into()))?,
    );

    Ok((
        StatusCode::CREATED,
        headers,
        Json(ProvisionResponse {
            participant_code: result.participant_code,
            agent_condition: result.agent_condition,
            team_id: result.team_id,
            project_id: result.project_id,
            access_token: result.access_token,
            token_type: "Bearer".into(),
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct EventsRequest {
    pub events: Vec<EventInput>,
}

#[derive(Debug, Serialize)]
pub struct EventsResponse {
    pub stored: usize,
}

/// Ingest a batch of behavioral telemetry events for the authenticated
/// participant. Study-mode only; the auth'd user must be a study participant.
pub async fn ingest_events(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<EventsRequest>,
) -> Result<Json<EventsResponse>, ApiError> {
    if !state.study_mode {
        return Err(ApiError::NotFound("Not found".into()));
    }
    let stored = study::telemetry::record_events(&state.db, auth.user_id, input.events).await?;
    Ok(Json(EventsResponse { stored }))
}

#[derive(Debug, Deserialize)]
pub struct SurveyRequest {
    pub responses: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub followup_email: Option<String>,
}

/// Persist the exit-survey responses for the authenticated participant and mark
/// them completed. Study-mode only.
pub async fn submit_survey(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<SurveyRequest>,
) -> Result<StatusCode, ApiError> {
    if !state.study_mode {
        return Err(ApiError::NotFound("Not found".into()));
    }
    study::survey::submit_survey(
        &state.db,
        auth.user_id,
        input.responses,
        input.followup_email,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct AgentRequest {
    pub message: String,
    #[serde(default)]
    pub current_task_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentResponse {
    pub reply: String,
    pub actions: Vec<String>,
    pub agent_condition: String,
}

/// One agent turn for the authenticated participant. Scoped to their condition
/// by the Context Gate. Study-mode only; requires a configured OpenAI key.
pub async fn agent_turn(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<AgentRequest>,
) -> Result<Json<AgentResponse>, ApiError> {
    if !state.study_mode {
        return Err(ApiError::NotFound("Not found".into()));
    }
    let agent = state
        .agent
        .as_ref()
        .ok_or_else(|| ApiError::Internal("The assistant is not configured.".into()))?;

    let ctx = study::agent::load_participant(&state.db, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::Forbidden("Not a study participant".into()))?;

    let result = study::agent::run::run_turn(
        &state.db,
        agent,
        &state.hub,
        &ctx,
        &input.message,
        input.current_task_id.as_deref(),
    )
    .await?;

    Ok(Json(AgentResponse {
        reply: result.reply,
        actions: result.actions,
        agent_condition: result.condition,
    }))
}
