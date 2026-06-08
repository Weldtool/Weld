use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::study;
use crate::services::study::export::{self, Dataset, StudySummary};
use crate::services::study::telemetry::EventInput;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_TYPE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
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
    /// True when an existing participant resumed via their access code rather
    /// than starting fresh; the frontend then restores the saved place below.
    pub resumed: bool,
    pub resume_stage: Option<String>,
    pub resume_task_index: i32,
}

#[derive(Debug, Default, Deserialize)]
pub struct ProvisionRequest {
    /// Version of the consent text the participant affirmed before provisioning.
    pub consent_version: Option<String>,
    /// Single-use access code. Required when `STUDY_REQUIRE_ACCESS_CODE` is set;
    /// ignored otherwise (so local testing needs no code).
    pub access_code: Option<String>,
    /// Participant's email address, collected for result linkage.
    pub participant_email: Option<String>,
    /// Participant's institution (university / company).
    pub institution: Option<String>,
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

    let (consent_version, access_code, participant_email, institution) = body
        .map(|Json(b)| {
            (
                b.consent_version,
                b.access_code,
                b.participant_email,
                b.institution,
            )
        })
        .unwrap_or((None, None, None, None));

    // Normalize: treat blank/whitespace as absent.
    let access_code = access_code
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    let participant_email = participant_email
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty());
    let institution = institution
        .map(|i| i.trim().to_string())
        .filter(|i| !i.is_empty());

    if state.study_require_access_code && access_code.is_none() {
        return Err(ApiError::Validation("An access code is required.".into()));
    }

    // When a code is supplied, an already-spent code resumes its participant
    // (any device, even after clearing storage); an unused one provisions fresh.
    let result = match access_code.as_deref() {
        Some(code) => match study::access_code::status(&state.db, code).await? {
            study::access_code::CodeStatus::Unknown => {
                return Err(ApiError::Validation("That access code isn't valid.".into()));
            }
            study::access_code::CodeStatus::Used { user_id } => {
                study::provision::resume_participant(&state.db, &state.jwt_secret, user_id).await?
            }
            study::access_code::CodeStatus::Unused => {
                study::provision_participant(
                    &state.db,
                    &state.jwt_secret,
                    consent_version.as_deref(),
                    Some(code),
                    participant_email.as_deref(),
                    institution.as_deref(),
                )
                .await?
            }
        },
        None => {
            study::provision_participant(
                &state.db,
                &state.jwt_secret,
                consent_version.as_deref(),
                None,
                participant_email.as_deref(),
                institution.as_deref(),
            )
            .await?
        }
    };

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
            resumed: result.resumed,
            resume_stage: result.resume_stage,
            resume_task_index: result.resume_task_index,
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
pub struct ProgressRequest {
    pub stage: String,
    pub task_index: i32,
}

/// Persist the authenticated participant's current place in the flow so they can
/// resume on another device via their access code. Study-mode only.
pub async fn save_progress(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<ProgressRequest>,
) -> Result<StatusCode, ApiError> {
    if !state.study_mode {
        return Err(ApiError::NotFound("Not found".into()));
    }
    study::provision::update_progress(&state.db, auth.user_id, &input.stage, input.task_index)
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
    /// Entities the agent touched this turn, so the client can attribute later
    /// user edits to them as verification/repair behavior (RQ1).
    pub entities: Vec<AgentEntityDto>,
    /// Chat messages the agent drafted for the participant to review and send.
    pub drafts: Vec<AgentDraftDto>,
    pub agent_condition: String,
}

#[derive(Debug, Serialize)]
pub struct AgentEntityDto {
    pub kind: String,
    pub id: String,
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct AgentDraftDto {
    pub channel: String,
    pub channel_id: String,
    pub content: String,
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
        entities: result
            .entities
            .into_iter()
            .map(|e| AgentEntityDto {
                kind: e.kind,
                id: e.id,
                action: e.action,
            })
            .collect(),
        drafts: result
            .drafts
            .into_iter()
            .map(|d| AgentDraftDto {
                channel: d.channel,
                channel_id: d.channel_id,
                content: d.content,
            })
            .collect(),
        agent_condition: result.condition,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    /// Which dataset: `participants`, `events`, or `survey`.
    pub dataset: String,
}

/// Export a research dataset as CSV for offline analysis. Study-mode only, and
/// guarded by `STUDY_EXPORT_TOKEN`: if that env var is unset the endpoint 404s
/// (disabled by default); otherwise the caller must present the matching token
/// in the `Authorization: Bearer` header. The token is deliberately *not*
/// accepted as a query param — query strings leak into access logs, proxies, and
/// browser history. Participant accounts can't reach it (they don't hold the
/// token). Follow-up contact PII is never exported.
pub async fn export_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    if !state.study_mode {
        return Err(ApiError::NotFound("Not found".into()));
    }
    // Disabled unless a token is configured.
    let configured = state
        .study_export_token
        .as_deref()
        .ok_or_else(|| ApiError::NotFound("Not found".into()))?;
    if bearer_token(&headers).as_deref() != Some(configured) {
        return Err(ApiError::Unauthorized("Invalid export token".into()));
    }

    let dataset = Dataset::parse(&q.dataset)?;
    let filename = dataset.filename();
    let csv = export::export_csv(&state.db, dataset).await?;

    let mut out = HeaderMap::new();
    out.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    out.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|_| ApiError::Internal("bad filename header".into()))?,
    );
    Ok((StatusCode::OK, out, csv).into_response())
}

#[derive(Debug, Deserialize)]
pub struct GenerateCodesRequest {
    /// How many codes to mint (1..=500).
    pub count: u32,
    /// Optional label for recruitment tracking (e.g. "pilot", "wave-2").
    #[serde(default)]
    pub note: Option<String>,
}

/// Mint single-use participation codes. Researcher-only: guarded by the same
/// `STUDY_EXPORT_TOKEN` as data export (404 when unset, 401 on mismatch). Returns
/// the new codes as `text/plain`, one per line, ready to hand out. Study-mode only.
pub async fn generate_codes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<GenerateCodesRequest>,
) -> Result<Response, ApiError> {
    if !state.study_mode {
        return Err(ApiError::NotFound("Not found".into()));
    }
    let configured = state
        .study_export_token
        .as_deref()
        .ok_or_else(|| ApiError::NotFound("Not found".into()))?;
    if bearer_token(&headers).as_deref() != Some(configured) {
        return Err(ApiError::Unauthorized("Invalid export token".into()));
    }

    if body.count == 0 || body.count > 500 {
        return Err(ApiError::Validation(
            "count must be between 1 and 500".into(),
        ));
    }

    let codes = study::access_code::generate(&state.db, body.count, body.note.as_deref()).await?;

    let mut out = HeaderMap::new();
    out.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    Ok((StatusCode::CREATED, out, codes.join("\n")).into_response())
}

/// Aggregate statistics for the researcher dashboard. Guarded by the same
/// `STUDY_EXPORT_TOKEN` as CSV export; 404 when unset, 401 on mismatch.
pub async fn get_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<StudySummary>, ApiError> {
    if !state.study_mode {
        return Err(ApiError::NotFound("Not found".into()));
    }
    let configured = state
        .study_export_token
        .as_deref()
        .ok_or_else(|| ApiError::NotFound("Not found".into()))?;
    if bearer_token(&headers).as_deref() != Some(configured) {
        return Err(ApiError::Unauthorized("Invalid export token".into()));
    }

    let summary = export::summary(&state.db).await?;
    Ok(Json(summary))
}

/// Extract a bearer token from the Authorization header, if present.
fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_string)
}
