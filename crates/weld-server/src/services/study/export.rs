//! CSV export and JSON summary of the research dataset.
//!
//! CSV datasets, each flat with `participant_code` + `agent_condition`
//! denormalized so a sheet stands alone:
//!   - `participants` — one row per participant (roster + lifecycle).
//!   - `events`       — one row per behavioral telemetry event (the bulk).
//!   - `survey`       — long format, one row per (participant, item).
//!   - `agent_turns`  — the agent conversation, one row per turn. The analysis
//!     plan names it as a source for correctness coding and the paper's H4
//!     calibration test needs it to code agent-error type; without it the
//!     transcripts stay in the database and neither analysis can be run.
//!
//! The follow-up contact table is deliberately NOT exportable here: it is
//! access-restricted PII and excluded from the analysis dataset by design
//! (WELD_EXIT_SURVEY.md §6).
//!
//! `summary()` returns aggregate statistics for the researcher dashboard.

use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

/// Which dataset to export. Parsed from the `?dataset=` query param.
pub enum Dataset {
    Participants,
    Events,
    Survey,
    AgentTurns,
    AccessCodes,
}

impl Dataset {
    pub fn parse(s: &str) -> Result<Self, ApiError> {
        match s {
            "participants" => Ok(Self::Participants),
            "events" => Ok(Self::Events),
            "survey" => Ok(Self::Survey),
            "agent_turns" => Ok(Self::AgentTurns),
            "access_codes" => Ok(Self::AccessCodes),
            other => Err(ApiError::Validation(format!(
                "unknown dataset '{other}' \
                 (expected participants|events|survey|agent_turns|access_codes)"
            ))),
        }
    }

    /// Suggested download filename.
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Participants => "study_participants.csv",
            Self::Events => "study_events.csv",
            Self::Survey => "study_survey.csv",
            Self::AgentTurns => "study_agent_turns.csv",
            Self::AccessCodes => "study_access_codes.csv",
        }
    }
}

/// Build the requested dataset as a CSV string.
pub async fn export_csv(pool: &PgPool, dataset: Dataset) -> Result<String, ApiError> {
    match dataset {
        Dataset::Participants => participants_csv(pool).await,
        Dataset::Events => events_csv(pool).await,
        Dataset::Survey => survey_csv(pool).await,
        Dataset::AgentTurns => agent_turns_csv(pool).await,
        Dataset::AccessCodes => access_codes_csv(pool).await,
    }
}

/// The agent conversation, one row per turn, ordered so a task reads in sequence.
///
/// `turn_index` is the position within (participant, task) so a coder can follow
/// the exchange without re-deriving order from timestamps. Content is the stored
/// text verbatim — this is the only dataset carrying free text the agent
/// produced, and the H4 calibration test is a judgement about that text.
async fn agent_turns_csv(pool: &PgPool) -> Result<String, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT p.participant_code, p.agent_condition,
                  t.task_id, t.role, t.content, t.created_at,
                  ROW_NUMBER() OVER (
                      PARTITION BY t.participant_id, t.task_id
                      ORDER BY t.created_at, t.id
                  ) AS "turn_index!"
           FROM study_agent_turns t
           JOIN study_participants p ON p.id = t.participant_id
           ORDER BY p.participant_code, t.task_id, t.created_at, t.id"#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("export query failed: {e}")))?;

    let mut out = String::new();
    write_row(
        &mut out,
        &[
            "participant_code",
            "agent_condition",
            "task_id",
            "turn_index",
            "role",
            "content",
            "created_at",
        ],
    );
    for r in rows {
        write_row(
            &mut out,
            &[
                &r.participant_code,
                &r.agent_condition,
                &r.task_id,
                &r.turn_index.to_string(),
                &r.role,
                &r.content,
                &r.created_at.to_rfc3339(),
            ],
        );
    }
    Ok(out)
}

/// Access-code roster: which codes exist, whether spent, and by whom. Lets the
/// researcher track redemption and spot unused codes. No participant PII.
async fn access_codes_csv(pool: &PgPool) -> Result<String, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT c.code, c.note, c.created_at, c.used_at,
                  p.participant_code AS "participant_code?"
           FROM study_access_codes c
           LEFT JOIN study_participants p ON p.id = c.participant_id
           ORDER BY c.created_at, c.code"#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("export query failed: {e}")))?;

    let mut out = String::new();
    write_row(
        &mut out,
        &["code", "note", "created_at", "used_at", "participant_code"],
    );
    for r in rows {
        write_row(
            &mut out,
            &[
                &r.code,
                &opt(r.note),
                &r.created_at.to_rfc3339(),
                &ts(r.used_at),
                &opt(r.participant_code),
            ],
        );
    }
    Ok(out)
}

async fn participants_csv(pool: &PgPool) -> Result<String, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT participant_code, agent_condition, status, consent_version,
                  consent_given_at, provisioned_at, completed_at,
                  user_id, team_id, project_id
           FROM study_participants
           ORDER BY participant_code"#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("export query failed: {e}")))?;

    let mut out = String::new();
    write_row(
        &mut out,
        &[
            "participant_code",
            "agent_condition",
            "status",
            "consent_version",
            "consent_given_at",
            "provisioned_at",
            "completed_at",
            "user_id",
            "team_id",
            "project_id",
        ],
    );
    for r in rows {
        write_row(
            &mut out,
            &[
                &r.participant_code,
                &r.agent_condition,
                &r.status,
                &opt(r.consent_version),
                &ts(r.consent_given_at),
                &r.provisioned_at.to_rfc3339(),
                &ts(r.completed_at),
                &r.user_id.to_string(),
                &r.team_id.to_string(),
                &r.project_id.to_string(),
            ],
        );
    }
    Ok(out)
}

async fn events_csv(pool: &PgPool) -> Result<String, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT p.participant_code, p.agent_condition,
                  e.id AS event_id, e.event_type, e.current_task_id,
                  e.payload, e.client_ts, e.server_ts
           FROM study_events e
           JOIN study_participants p ON p.id = e.participant_id
           ORDER BY p.participant_code, e.server_ts, e.id"#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("export query failed: {e}")))?;

    let mut out = String::new();
    write_row(
        &mut out,
        &[
            "participant_code",
            "agent_condition",
            "event_id",
            "event_type",
            "current_task_id",
            "client_ts",
            "server_ts",
            "payload",
        ],
    );
    for r in rows {
        write_row(
            &mut out,
            &[
                &r.participant_code,
                &r.agent_condition,
                &r.event_id.to_string(),
                &r.event_type,
                &opt(r.current_task_id),
                &ts(r.client_ts),
                &r.server_ts.to_rfc3339(),
                &json_str(&r.payload),
            ],
        );
    }
    Ok(out)
}

async fn survey_csv(pool: &PgPool) -> Result<String, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT p.participant_code, p.agent_condition,
                  s.item_id, s.value, s.answered_at
           FROM study_survey_responses s
           JOIN study_participants p ON p.id = s.participant_id
           ORDER BY p.participant_code, s.item_id"#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("export query failed: {e}")))?;

    let mut out = String::new();
    write_row(
        &mut out,
        &[
            "participant_code",
            "agent_condition",
            "item_id",
            "value",
            "answered_at",
        ],
    );
    for r in rows {
        write_row(
            &mut out,
            &[
                &r.participant_code,
                &r.agent_condition,
                &r.item_id,
                &r.value,
                &r.answered_at.to_rfc3339(),
            ],
        );
    }
    Ok(out)
}

// ── JSON summary for the researcher dashboard ───────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ConditionCount {
    pub condition: String,
    pub total: i64,
    pub completed: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventTypeCount {
    pub event_type: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParticipantRow {
    pub participant_code: String,
    pub agent_condition: String,
    pub status: String,
    pub provisioned_at: String,
    pub completed_at: Option<String>,
    /// Session duration in seconds, if completed.
    pub duration_seconds: Option<i64>,
    pub event_count: i64,
    pub survey_items_answered: i64,
    /// Server receipt time of the participant's most recent telemetry event, if
    /// any. Lets the dashboard flag stale `in_progress` rows (likely dropouts) —
    /// nothing sets status to `abandoned`, so a dropout otherwise looks active.
    pub last_event_at: Option<String>,
    /// Sum of active (engaged) time across this participant's completed tasks,
    /// from `task_completed.task_time_ms`. The real time-on-task — unlike
    /// `duration_seconds` (wall clock incl. breaks/resumes).
    pub active_task_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessCodeStats {
    pub total: i64,
    pub used: i64,
    pub unused: i64,
}

/// One cell of the headline figure: mean active time on an agent task, by
/// condition × task type (the Fig. 1 interaction shape). Descriptive preview of
/// raw active time — the paper normalizes per participant; this does not.
#[derive(Debug, Serialize, Deserialize)]
pub struct TimeCell {
    pub condition: String,
    pub task_type: String,
    pub mean_active_ms: i64,
    pub n: i64,
}

/// Division of labor by condition (Fig. 4 shape): agent-executed actions vs.
/// user actions during the agent phase.
#[derive(Debug, Serialize, Deserialize)]
pub struct DelegationRow {
    pub condition: String,
    pub agent_actions: i64,
    pub user_actions: i64,
}

/// Live scoping manipulation check (Fig. 5): from `agent_context_sent`, the
/// share of turns whose snapshot contained no disallowed source. C1 turns should
/// be 100% board+GitHub-free, C2 turns 100% GitHub-free.
#[derive(Debug, Serialize, Deserialize)]
pub struct ManipulationRow {
    pub condition: String,
    pub turns: i64,
    /// Turns with neither a board nor a github key in the snapshot (C1 target).
    pub clean_no_board_github: i64,
    /// Turns with no github key in the snapshot (C2 target).
    pub clean_no_github: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StudyCharts {
    pub time_by_cell: Vec<TimeCell>,
    pub delegation: Vec<DelegationRow>,
    pub manipulation: Vec<ManipulationRow>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StudySummary {
    pub total_participants: i64,
    pub by_condition: Vec<ConditionCount>,
    pub by_status: Vec<StatusCount>,
    pub top_event_types: Vec<EventTypeCount>,
    pub access_codes: AccessCodeStats,
    pub charts: StudyCharts,
    pub participants: Vec<ParticipantRow>,
}

/// Aggregate statistics for the researcher dashboard UI.
pub async fn summary(pool: &PgPool) -> Result<StudySummary, ApiError> {
    let err = |e: sqlx::Error| ApiError::Internal(format!("summary query failed: {e}"));

    let total_participants = sqlx::query_scalar!("SELECT COUNT(*)::bigint FROM study_participants")
        .fetch_one(pool)
        .await
        .map_err(err)?
        .unwrap_or(0);

    let by_condition_rows = sqlx::query!(
        r#"SELECT agent_condition,
                  COUNT(*)::bigint AS "total!",
                  COUNT(*) FILTER (WHERE status = 'completed')::bigint AS "completed!"
           FROM study_participants
           GROUP BY agent_condition
           ORDER BY agent_condition"#
    )
    .fetch_all(pool)
    .await
    .map_err(err)?;

    let by_condition = by_condition_rows
        .into_iter()
        .map(|r| ConditionCount {
            condition: r.agent_condition,
            total: r.total,
            completed: r.completed,
        })
        .collect();

    let by_status_rows = sqlx::query!(
        r#"SELECT status, COUNT(*)::bigint AS "n!"
           FROM study_participants
           GROUP BY status
           ORDER BY 2 DESC"#
    )
    .fetch_all(pool)
    .await
    .map_err(err)?;

    let by_status = by_status_rows
        .into_iter()
        .map(|r| StatusCount {
            status: r.status,
            count: r.n,
        })
        .collect();

    let top_event_types = sqlx::query!(
        r#"SELECT event_type, COUNT(*)::bigint AS "n!"
           FROM study_events
           GROUP BY event_type
           ORDER BY 2 DESC
           LIMIT 20"#
    )
    .fetch_all(pool)
    .await
    .map_err(err)?
    .into_iter()
    .map(|r| EventTypeCount {
        event_type: r.event_type,
        count: r.n,
    })
    .collect();

    let access_code_row = sqlx::query!(
        r#"SELECT COUNT(*)::bigint AS "total!",
                  COUNT(*) FILTER (WHERE used_at IS NOT NULL)::bigint AS "used!",
                  COUNT(*) FILTER (WHERE used_at IS NULL)::bigint AS "unused!"
           FROM study_access_codes"#
    )
    .fetch_one(pool)
    .await
    .map_err(err)?;

    let access_codes = AccessCodeStats {
        total: access_code_row.total,
        used: access_code_row.used,
        unused: access_code_row.unused,
    };

    let participant_rows = sqlx::query!(
        // Survey items are a scalar subquery, NOT a second LEFT JOIN: joining both
        // study_events and study_survey_responses fans out (Cartesian) and would
        // multiply the SUM below by the survey-row count. COUNT(DISTINCT) hides
        // that on the counts, but the un-DISTINCT-able SUM(task_time_ms) would be
        // inflated ~N-survey-rows-fold.
        r#"SELECT p.participant_code, p.agent_condition, p.status,
                  p.provisioned_at, p.completed_at,
                  EXTRACT(EPOCH FROM (p.completed_at - p.provisioned_at))::bigint AS duration_seconds,
                  COUNT(DISTINCT e.id)::bigint AS "event_count!",
                  (SELECT COUNT(DISTINCT s.item_id)
                     FROM study_survey_responses s
                    WHERE s.participant_id = p.id)::bigint AS "survey_items!",
                  MAX(e.server_ts) AS last_event_at,
                  SUM((e.payload->>'task_time_ms')::bigint)
                    FILTER (WHERE e.event_type = 'task_completed')::bigint AS active_task_ms
           FROM study_participants p
           LEFT JOIN study_events e ON e.participant_id = p.id
           GROUP BY p.id
           ORDER BY p.participant_code"#
    )
    .fetch_all(pool)
    .await
    .map_err(err)?;

    let participants = participant_rows
        .into_iter()
        .map(|r| ParticipantRow {
            participant_code: r.participant_code,
            agent_condition: r.agent_condition,
            status: r.status,
            provisioned_at: r.provisioned_at.to_rfc3339(),
            completed_at: r.completed_at.map(|t| t.to_rfc3339()),
            duration_seconds: r.duration_seconds,
            event_count: r.event_count,
            survey_items_answered: r.survey_items,
            last_event_at: r.last_event_at.map(|t| t.to_rfc3339()),
            active_task_ms: r.active_task_ms,
        })
        .collect();

    let charts = charts(pool).await?;

    Ok(StudySummary {
        total_participants,
        by_condition,
        by_status,
        top_event_types,
        access_codes,
        charts,
        participants,
    })
}

/// Descriptive chart aggregates for the dashboard's default view — previews of
/// the paper figures, computed live from telemetry. Correctness (Fig. 2) and the
/// trust/perception figure (Fig. 3) are intentionally absent: correctness is
/// hand-coded offline and perception needs the analyzed survey, so neither is
/// derivable from the raw event log.
async fn charts(pool: &PgPool) -> Result<StudyCharts, ApiError> {
    let err = |e: sqlx::Error| ApiError::Internal(format!("charts query failed: {e}"));

    // Fig. 1 shape — mean active time per condition × task type, agent tasks only
    // (task ids T1..T6). Raw active ms; the paper normalizes per participant.
    let time_by_cell = sqlx::query!(
        r#"SELECT p.agent_condition AS "condition!",
                  e.payload->>'task_type' AS "task_type!",
                  AVG((e.payload->>'task_time_ms')::bigint)::bigint AS "mean_active_ms!",
                  COUNT(*)::bigint AS "n!"
           FROM study_events e
           JOIN study_participants p ON p.id = e.participant_id
           WHERE e.event_type = 'task_completed'
             AND e.payload->>'task_id' LIKE 'T%'
             AND e.payload->>'task_time_ms' IS NOT NULL
             AND e.payload->>'task_type' IS NOT NULL
           GROUP BY p.agent_condition, e.payload->>'task_type'
           ORDER BY p.agent_condition, e.payload->>'task_type'"#
    )
    .fetch_all(pool)
    .await
    .map_err(err)?
    .into_iter()
    .map(|r| TimeCell {
        condition: r.condition,
        task_type: r.task_type,
        mean_active_ms: r.mean_active_ms,
        n: r.n,
    })
    .collect();

    // Fig. 4 shape — agent-executed vs. user actions during the agent phase
    // (current_task_id T1..T6), by condition.
    let delegation = sqlx::query!(
        r#"SELECT p.agent_condition AS "condition!",
                  COUNT(*) FILTER (
                      WHERE e.event_type = 'agent_tool_call'
                        AND e.payload->>'executed' = 'true'
                        -- A drafted chat message is not an agent-completed
                        -- action: the participant reviews and sends it, which
                        -- already lands as a `message_sent` user action. Counting
                        -- both would put one outcome on both sides of the ratio.
                        AND COALESCE(e.payload->>'drafted', 'false') <> 'true'
                  )::bigint AS "agent_actions!",
                  COUNT(*) FILTER (
                      WHERE e.event_type IN ('card_created','card_moved','card_edited',
                                             'card_priority_set','card_assigned','card_labeled',
                                             'message_sent')
                  )::bigint AS "user_actions!"
           FROM study_events e
           JOIN study_participants p ON p.id = e.participant_id
           WHERE e.current_task_id LIKE 'T%'
           GROUP BY p.agent_condition
           ORDER BY p.agent_condition"#
    )
    .fetch_all(pool)
    .await
    .map_err(err)?
    .into_iter()
    .map(|r| DelegationRow {
        condition: r.condition,
        agent_actions: r.agent_actions,
        user_actions: r.user_actions,
    })
    .collect();

    // Fig. 5 — live scoping proof from agent_context_sent snapshots. The `?`
    // operator tests jsonb key existence: C1 turns should carry neither board nor
    // github; C2 turns no github.
    let manipulation = sqlx::query!(
        r#"SELECT p.agent_condition AS "condition!",
                  COUNT(*)::bigint AS "turns!",
                  COUNT(*) FILTER (
                      WHERE NOT jsonb_exists(e.payload, 'board')
                        AND NOT jsonb_exists(e.payload, 'github')
                  )::bigint AS "clean_no_board_github!",
                  COUNT(*) FILTER (
                      WHERE NOT jsonb_exists(e.payload, 'github')
                  )::bigint AS "clean_no_github!"
           FROM study_events e
           JOIN study_participants p ON p.id = e.participant_id
           WHERE e.event_type = 'agent_context_sent'
           GROUP BY p.agent_condition
           ORDER BY p.agent_condition"#
    )
    .fetch_all(pool)
    .await
    .map_err(err)?
    .into_iter()
    .map(|r| ManipulationRow {
        condition: r.condition,
        turns: r.turns,
        clean_no_board_github: r.clean_no_board_github,
        clean_no_github: r.clean_no_github,
    })
    .collect();

    Ok(StudyCharts {
        time_by_cell,
        delegation,
        manipulation,
    })
}

// ── CSV helpers ─────────────────────────────────────────────────────────────

/// Write one CSV record (RFC 4180 quoting) followed by CRLF.
fn write_row(out: &mut String, fields: &[&str]) {
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&escape(f));
    }
    out.push_str("\r\n");
}

/// Render a field for CSV: first neutralize spreadsheet formula injection, then
/// apply RFC 4180 quoting.
fn escape(field: &str) -> String {
    let defused = defuse_formula(field);
    if defused.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", defused.replace('"', "\"\""))
    } else {
        defused
    }
}

/// Excel/Sheets treat a cell beginning with `=`, `+`, `-`, `@`, tab, or CR as a
/// formula. User-derived values (survey free-text, snapshot message content,
/// task ids) could start with one of these, so prefix a single quote to force
/// the cell to be read as text. Applied to every field, which is harmless for
/// values that don't start with a trigger.
fn defuse_formula(field: &str) -> String {
    match field.chars().next() {
        Some('=') | Some('+') | Some('-') | Some('@') | Some('\t') | Some('\r') => {
            format!("'{field}")
        }
        _ => field.to_string(),
    }
}

fn opt(v: Option<String>) -> String {
    v.unwrap_or_default()
}

fn ts(v: Option<chrono::DateTime<chrono::Utc>>) -> String {
    v.map(|t| t.to_rfc3339()).unwrap_or_default()
}

fn json_str(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}
