//! Telemetry ingest for the study (Category A behavioral events,
//! paper/instrument/CLAUDE_STUDY.md §5/§5A). Events are produced client-side as
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
    // Post-baseline NASA-TLX interstitial (baseline→agent transition).
    "interim_tlx_shown",
    "interim_tlx_submitted",
    // Per-task timing
    "task_shown",
    "task_completed",
    "task_skipped",
    "task_final_state",
    "task_rating",
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
    // Cross-reference (headline integration metric). Only the directions with a
    // UI affordance are emitted: chat→card, chat→PR, card→PR (plus card→issue
    // below). Two are reserved but have no affordance in the current app —
    // `chat_referenced_from_card` (cards don't surface clickable chat refs) and
    // `card_referenced_from_pr` (the PR review view doesn't link back to a card).
    // Treat both as N/A in analysis, not as missing data: `CROSS_REFERENCE_EVENTS`
    // in `paper/analysis/derive.py` lists all six, so the cross-reference count
    // is a sum over four live directions, identically for every condition.
    "card_referenced_from_chat",
    "chat_referenced_from_card",
    "pr_referenced_from_card",
    "pr_referenced_from_chat",
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
    // Agent
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
    "agent_panel_opened",
    "agent_panel_collapsed",
    // Server-side backstop firing: the model echoed a raw card id and we
    // rewrote it. Recorded so the rate is auditable rather than invisible.
    "agent_id_leak_sanitized",
    // GitHub cross-reference (from card)
    "issue_referenced_from_card",
    // In-app PR review (B4 / T6). `pr_review_submitted` carries the verdict and
    // the participant's review comment — that comment is never written to
    // GitHub, so this event is its only record and the rubric needs it.
    "pr_review_opened",
    "pr_review_submitted",
    // Merging is the act that ships a PR (approving only signals), so it is
    // recorded separately from the review verdict.
    "pr_merged",
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
/// Returns the number of events stored.
///
/// Events with an unrecognised type are **dropped and logged**, not stored, and
/// the rest of the batch is still written. Rejecting the whole batch would be
/// worse than useless here: the client re-queues a failed batch, so a single
/// unknown type would poison the buffer and silently cost the participant every
/// subsequent event of the session. The privacy guarantee is unchanged — an
/// unknown type still never reaches the database.
pub async fn record_events(
    pool: &PgPool,
    user_id: Uuid,
    events: Vec<EventInput>,
) -> Result<usize, ApiError> {
    let participant_id = participant_for_user(pool, user_id)
        .await?
        .ok_or_else(|| ApiError::Forbidden("Not a study participant".into()))?;

    let (events, rejected): (Vec<_>, Vec<_>) = events
        .into_iter()
        .partition(|ev| ALLOWED_EVENT_TYPES.contains(&ev.event_type.as_str()));

    for ev in &rejected {
        tracing::warn!(
            event_type = %ev.event_type,
            %participant_id,
            "Dropped telemetry event with unknown type — instrumented in the client but missing from ALLOWED_EVENT_TYPES?"
        );
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
