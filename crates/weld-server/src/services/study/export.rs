//! CSV export of the research dataset for offline analysis (R/pandas/Excel).
//!
//! Three datasets, each a flat CSV with `participant_code` + `agent_condition`
//! denormalized in so a sheet stands alone:
//!   - `participants` — one row per participant (roster + lifecycle).
//!   - `events`       — one row per behavioral telemetry event (the bulk).
//!   - `survey`       — long format, one row per (participant, item).
//!
//! The follow-up contact table is deliberately NOT exportable here: it is
//! access-restricted PII and excluded from the analysis dataset by design
//! (WELD_EXIT_SURVEY.md §6).

use crate::errors::ApiError;
use serde_json::Value;
use sqlx::PgPool;

/// Which dataset to export. Parsed from the `?dataset=` query param.
pub enum Dataset {
    Participants,
    Events,
    Survey,
}

impl Dataset {
    pub fn parse(s: &str) -> Result<Self, ApiError> {
        match s {
            "participants" => Ok(Self::Participants),
            "events" => Ok(Self::Events),
            "survey" => Ok(Self::Survey),
            other => Err(ApiError::Validation(format!(
                "unknown dataset '{other}' (expected participants|events|survey)"
            ))),
        }
    }

    /// Suggested download filename.
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Participants => "study_participants.csv",
            Self::Events => "study_events.csv",
            Self::Survey => "study_survey.csv",
        }
    }
}

/// Build the requested dataset as a CSV string.
pub async fn export_csv(pool: &PgPool, dataset: Dataset) -> Result<String, ApiError> {
    match dataset {
        Dataset::Participants => participants_csv(pool).await,
        Dataset::Events => events_csv(pool).await,
        Dataset::Survey => survey_csv(pool).await,
    }
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
