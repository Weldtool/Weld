//! Telemetry ingest for the study (Category A behavioral events,
//! research-paper/CLAUDE_STUDY.md §5/§5A). Events are produced client-side as
//! the participant works and batched here. Privacy is enforced by construction:
//! we accept only the fixed allowlist of event types below — there is no
//! free-form firehose, so keystrokes/mouse/screen/other-tab data can't be
//! ingested even if a client tried to send them.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;

/// Every event type the study may record (§5). Anything else is rejected.
/// Agent + survey/consent lifecycle types are included so the spine is ready
/// for those phases without a schema change.
const ALLOWED_EVENT_TYPES: &[&str] = &[
    // Session & lifecycle
    "session_start",
    "session_end",
    "session_abandoned",
    "session_resumed",
    "phase_started",
    "phase_ended",
    "consent_given",
    "survey_started",
    "survey_completed",
    // Per-task timing
    "task_shown",
    "task_completed",
    "task_skipped",
    "task_final_state",
    "instruction_reopened",
    // Board
    "card_created",
    "card_opened",
    "card_edited",
    "card_moved",
    "card_assigned",
    "card_labeled",
    "card_priority_set",
    "card_deleted",
    "card_searched",
    "board_filtered",
    // Chat
    "message_sent",
    "message_edited",
    "message_deleted",
    "channel_switched",
    "thread_opened",
    "mention_clicked",
    "link_clicked",
    // Cross-reference (headline integration metric)
    "card_referenced_from_chat",
    "chat_referenced_from_card",
    "pr_referenced_from_card",
    "card_referenced_from_pr",
    // GitHub
    "github_view_opened",
    "pr_opened",
    "issue_opened",
    "pr_linked_to_card",
    "issue_linked_to_card",
    // Navigation
    "view_changed",
    "search_used",
    // Agent (future phase)
    "agent_invoked",
    "agent_context_sent",
    "agent_response",
    "agent_tool_call",
    "agent_tool_rejected",
    "agent_suggestion_accepted",
    "agent_suggestion_rejected",
    "agent_action_undone",
    "agent_output_edited",
    "agent_error",
];

#[derive(Debug, Deserialize)]
pub struct EventInput {
    pub event_type: String,
    #[serde(default)]
    pub current_task_id: Option<String>,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub client_ts: Option<DateTime<Utc>>,
}

/// Record a single event server-side (e.g. agent events), bypassing the
/// client batch path. The caller already holds the participant id. Best-effort:
/// a telemetry write must never fail the action that produced it, so errors are
/// logged and swallowed.
pub async fn record_server_event(
    pool: &PgPool,
    participant_id: Uuid,
    event_type: &str,
    current_task_id: Option<&str>,
    payload: Value,
) {
    let result = sqlx::query!(
        r#"
        INSERT INTO study_events (participant_id, event_type, current_task_id, payload, client_ts)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        participant_id,
        event_type,
        current_task_id,
        payload,
    )
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::error!(?e, event_type, "failed to record server telemetry event");
    }
}

/// Resolve the participant owning this user account, or `None` if the user
/// isn't a study participant (e.g. a normal account hitting the endpoint).
async fn participant_for_user(pool: &PgPool, user_id: Uuid) -> Result<Option<Uuid>, ApiError> {
    let row = sqlx::query_scalar!(
        "SELECT id FROM study_participants WHERE user_id = $1",
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Persist a batch of behavioral events for the authenticated participant.
/// Returns the number of events stored. Unknown event types are rejected.
pub async fn record_events(
    pool: &PgPool,
    user_id: Uuid,
    events: Vec<EventInput>,
) -> Result<usize, ApiError> {
    let participant_id = participant_for_user(pool, user_id)
        .await?
        .ok_or_else(|| ApiError::Forbidden("Not a study participant".into()))?;

    for ev in &events {
        if !ALLOWED_EVENT_TYPES.contains(&ev.event_type.as_str()) {
            return Err(ApiError::Validation(format!(
                "Unknown telemetry event type: {}",
                ev.event_type
            )));
        }
    }

    let mut tx = pool.begin().await?;
    for ev in &events {
        let payload = ev
            .payload
            .clone()
            .unwrap_or_else(|| Value::Object(Default::default()));
        sqlx::query!(
            r#"
            INSERT INTO study_events (participant_id, event_type, current_task_id, payload, client_ts)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            participant_id,
            ev.event_type,
            ev.current_task_id,
            payload,
            ev.client_ts,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(events.len())
}
