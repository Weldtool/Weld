//! Single-use access codes that cap study participation to one run per
//! recruited person (see `research-paper/`). Codes are pre-minted by the
//! researcher and handed out one-per-participant; provisioning validates and
//! spends a code in the same transaction that creates the participant, so a
//! code can never provision twice even under concurrent requests.

use rand::Rng;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::errors::ApiError;

/// Characters used in generated codes — no 0/O/1/I/L to avoid transcription
/// errors when a participant types the code by hand.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Generate `count` fresh codes (formatted `WELD-XXXX-XXXX`) and persist them.
/// Returns the codes so the caller can hand them out. Duplicates are vanishingly
/// unlikely but skipped on conflict so one collision can't fail the batch.
pub async fn generate(
    pool: &sqlx::PgPool,
    count: u32,
    note: Option<&str>,
) -> Result<Vec<String>, ApiError> {
    let mut out = Vec::with_capacity(count as usize);
    let mut attempts = 0u32;
    while out.len() < count as usize {
        attempts += 1;
        if attempts > count.saturating_mul(4) + 16 {
            return Err(ApiError::Internal(
                "Could not generate enough unique codes".into(),
            ));
        }
        let code = random_code();
        let inserted = sqlx::query_scalar!(
            "INSERT INTO study_access_codes (code, note) VALUES ($1, $2)
             ON CONFLICT (code) DO NOTHING RETURNING code",
            code,
            note,
        )
        .fetch_optional(pool)
        .await?;
        if let Some(c) = inserted {
            out.push(c);
        }
    }
    Ok(out)
}

/// Redemption state of a code, used to decide between a fresh provision and a
/// resume of an existing participant.
pub enum CodeStatus {
    /// No such code exists.
    Unknown,
    /// Valid and not yet spent — provision a new participant.
    Unused,
    /// Already spent — resume the linked participant's session.
    Used { user_id: Uuid },
}

/// Look up a code's redemption state (no lock; the consuming path re-checks
/// under `FOR UPDATE`). A spent code whose participant row was deleted reports
/// `Unknown` so it can't resume a non-existent session.
pub async fn status(pool: &sqlx::PgPool, code: &str) -> Result<CodeStatus, ApiError> {
    let row = sqlx::query!(
        r#"SELECT c.used_at, p.user_id AS "user_id?"
           FROM study_access_codes c
           LEFT JOIN study_participants p ON p.id = c.participant_id
           WHERE c.code = $1"#,
        code,
    )
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        None => CodeStatus::Unknown,
        Some(r) if r.used_at.is_none() => CodeStatus::Unused,
        Some(r) => match r.user_id {
            Some(user_id) => CodeStatus::Used { user_id },
            None => CodeStatus::Unknown,
        },
    })
}

/// Validate a code and lock its row for the rest of the transaction. Errors if
/// the code is unknown (`Validation`) or already spent (`Conflict`). The lock is
/// held until commit/rollback, so a concurrent provision with the same code
/// blocks here and then sees it spent — closing the double-use race.
pub async fn lock_unused(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
) -> Result<(), ApiError> {
    let row = sqlx::query!(
        "SELECT used_at FROM study_access_codes WHERE code = $1 FOR UPDATE",
        code,
    )
    .fetch_optional(&mut **tx)
    .await?;

    match row {
        None => Err(ApiError::Validation("That access code isn't valid.".into())),
        Some(r) if r.used_at.is_some() => Err(ApiError::Conflict(
            "That access code has already been used.".into(),
        )),
        Some(_) => Ok(()),
    }
}

/// Spend a (already-locked) code, linking it to the participant it provisioned.
pub async fn mark_used(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
    participant_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE study_access_codes SET used_at = NOW(), participant_id = $2 WHERE code = $1",
        code,
        participant_id,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn random_code() -> String {
    let mut rng = rand::thread_rng();
    let group = |rng: &mut rand::rngs::ThreadRng| -> String {
        (0..4)
            .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
            .collect()
    };
    format!("WELD-{}-{}", group(&mut rng), group(&mut rng))
}
