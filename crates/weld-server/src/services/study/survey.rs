//! Exit-survey persistence (Category B, paper/replication/WELD_EXIT_SURVEY.md).
//! Responses are stored one row per item; the optional follow-up email goes to
//! a separate access-restricted table so it never enters the analysis dataset.

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;

async fn participant_for_user(pool: &PgPool, user_id: Uuid) -> Result<Uuid, ApiError> {
    sqlx::query_scalar!(
        "SELECT id FROM study_participants WHERE user_id = $1",
        user_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::Forbidden("Not a study participant".into()))
}

/// Persist an interim instrument (the post-baseline NASA-TLX) for the
/// authenticated participant. Unlike [`submit_survey`], this does NOT mark the
/// participant completed — they are mid-study, about to start the agent phase.
/// Item ids are pre-suffixed by the client (e.g. `tlx_mental_baseline`) so these
/// rows coexist with the exit-survey TLX in `study_survey_responses`.
pub async fn submit_interim_survey(
    pool: &PgPool,
    user_id: Uuid,
    responses: HashMap<String, String>,
) -> Result<(), ApiError> {
    let participant_id = participant_for_user(pool, user_id).await?;

    let mut tx = pool.begin().await?;
    for (item_id, value) in &responses {
        sqlx::query!(
            r#"
            INSERT INTO study_survey_responses (participant_id, item_id, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (participant_id, item_id)
            DO UPDATE SET value = EXCLUDED.value, answered_at = NOW()
            "#,
            participant_id,
            item_id,
            value,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Persist the exit-survey responses for the authenticated participant, store
/// any follow-up email separately, and mark the participant completed.
pub async fn submit_survey(
    pool: &PgPool,
    user_id: Uuid,
    responses: HashMap<String, String>,
    followup_email: Option<String>,
) -> Result<(), ApiError> {
    let participant_id = participant_for_user(pool, user_id).await?;

    let mut tx = pool.begin().await?;

    for (item_id, value) in &responses {
        sqlx::query!(
            r#"
            INSERT INTO study_survey_responses (participant_id, item_id, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (participant_id, item_id)
            DO UPDATE SET value = EXCLUDED.value, answered_at = NOW()
            "#,
            participant_id,
            item_id,
            value,
        )
        .execute(&mut *tx)
        .await?;
    }

    // Follow-up contact lives in its own access-restricted table, never joined
    // into analysis exports (WELD_EXIT_SURVEY.md §6).
    if let Some(email) = followup_email
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        sqlx::query!(
            r#"
            INSERT INTO study_followup_contacts (participant_id, email)
            VALUES ($1, $2)
            ON CONFLICT (participant_id) DO UPDATE SET email = EXCLUDED.email
            "#,
            participant_id,
            email,
        )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
        "UPDATE study_participants SET status = 'completed', completed_at = NOW() WHERE id = $1",
        participant_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}
