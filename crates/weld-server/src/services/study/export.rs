//! CSV export and JSON summary of the research dataset.
//!
//! Three CSV datasets, each flat with `participant_code` + `agent_condition`
//! denormalized so a sheet stands alone:
//!   - `participants` — one row per participant (roster + lifecycle).
//!   - `events`       — one row per behavioral telemetry event (the bulk).
//!   - `survey`       — long format, one row per (participant, item).
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
    AccessCodes,
}

impl Dataset {
    pub fn parse(s: &str) -> Result<Self, ApiError> {
        match s {
            "participants" => Ok(Self::Participants),
            "events" => Ok(Self::Events),
            "survey" => Ok(Self::Survey),
            "access_codes" => Ok(Self::AccessCodes),
            other => Err(ApiError::Validation(format!(
                "unknown dataset '{other}' (expected participants|events|survey|access_codes)"
            ))),
        }
    }

    /// Suggested download filename.
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Participants => "study_participants.csv",
            Self::Events => "study_events.csv",
            Self::Survey => "study_survey.csv",
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
        Dataset::AccessCodes => access_codes_csv(pool).await,
    }
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessCodeStats {
    pub total: i64,
    pub used: i64,
    pub unused: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StudySummary {
    pub total_participants: i64,
    pub by_condition: Vec<ConditionCount>,
    pub by_status: Vec<StatusCount>,
    pub top_event_types: Vec<EventTypeCount>,
    pub access_codes: AccessCodeStats,
    pub participants: Vec<ParticipantRow>,
}

/// Aggregate statistics for the researcher dashboard UI.
pub async fn summary(pool: &PgPool) -> Result<StudySummary, ApiError> {
    let err = |e: sqlx::Error| ApiError::Internal(format!("summary query failed: {e}"));

    let total_participants = sqlx::query_scalar!(
        "SELECT COUNT(*)::bigint FROM study_participants"
    )
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
        r#"SELECT p.participant_code, p.agent_condition, p.status,
                  p.provisioned_at, p.completed_at,
                  EXTRACT(EPOCH FROM (p.completed_at - p.provisioned_at))::bigint AS duration_seconds,
                  COUNT(DISTINCT e.id)::bigint AS "event_count!",
                  COUNT(DISTINCT s.item_id)::bigint AS "survey_items!"
           FROM study_participants p
           LEFT JOIN study_events e ON e.participant_id = p.id
           LEFT JOIN study_survey_responses s ON s.participant_id = p.id
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
        })
        .collect();

    Ok(StudySummary {
        total_participants,
        by_condition,
        by_status,
        top_event_types,
        access_codes,
        participants,
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
