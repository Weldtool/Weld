//! Participant provisioning: clone the PetTracker seed into a fresh, isolated
//! study team so each participant's actions are invisible to every other
//! participant (per `paper/instrument/CLAUDE_STUDY.md` §2.2). One transaction —
//! either the whole isolated workspace exists or none of it does.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

use super::access_code;
use super::seed::{self, Seed};
use crate::errors::ApiError;
use crate::services::auth::helpers as auth_helpers;

pub struct ProvisionResult {
    pub participant_code: String,
    pub agent_condition: String,
    pub user_id: Uuid,
    pub team_id: Uuid,
    pub project_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    /// True when this is a resume of an existing participant (re-entered code)
    /// rather than a fresh provision.
    pub resumed: bool,
    /// Saved stage to resume into (e.g. "tasks"), when resuming.
    pub resume_stage: Option<String>,
    /// Saved task index to resume at, when resuming.
    pub resume_task_index: i32,
    /// Whether the post-baseline NASA-TLX has already been answered. Read on
    /// resume so a participant who comes back on a new device at the
    /// baseline→agent boundary is still shown it; without it their
    /// `tlx_*_baseline` rows never exist and H3's delta can't be derived.
    pub baseline_tlx_done: bool,
}

/// Provision a brand-new participant: allocate a code, assign an agent
/// condition, create the participant user + isolated cloned PetTracker team,
/// and issue session tokens.
pub async fn provision_participant(
    pool: &PgPool,
    jwt_secret: &str,
    consent_version: Option<&str>,
    access_code: Option<&str>,
    participant_email: Option<&str>,
    institution: Option<&str>,
) -> Result<ProvisionResult, ApiError> {
    let seed =
        Seed::load().map_err(|e| ApiError::Internal(format!("Invalid study seed fixture: {e}")))?;

    let mut tx = pool.begin().await?;

    // Validate + lock the access code before doing any work, so an invalid or
    // spent code fails fast and a concurrent duplicate blocks until we commit.
    if let Some(code) = access_code {
        access_code::lock_unused(&mut tx, code).await?;
    }

    // After the code check, so a bad code doesn't queue behind the assignment
    // lock, and inside the transaction so the count it reads can't go stale.
    let condition = pick_agent_condition(&mut tx).await?;

    // --- Participant code + user ---
    let code_num = sqlx::query_scalar!("SELECT nextval('study_participant_code_seq')")
        .fetch_one(&mut *tx)
        .await?
        .unwrap_or(1);
    let code = format!("P{code_num:04}");
    let slug_suffix = code.to_lowercase();

    let participant_id = sqlx::query_scalar!(
        r#"
        INSERT INTO users (email, display_name, password_hash, role)
        VALUES ($1, 'You', '', 'member')
        RETURNING id
        "#,
        format!("participant-{slug_suffix}@study.weld.internal"),
    )
    .fetch_one(&mut *tx)
    .await?;

    // --- Isolated study team (participant = Owner, teammates = Members) ---
    let team_id = sqlx::query_scalar!(
        r#"
        INSERT INTO teams (name, slug, owner_id, is_study)
        VALUES ($1, $2, $3, TRUE)
        RETURNING id
        "#,
        "PetTracker",
        format!("pettracker-study-{slug_suffix}"),
        participant_id,
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'owner')",
        team_id,
        participant_id,
    )
    .execute(&mut *tx)
    .await?;
    for teammate in seed::TEAMMATES {
        sqlx::query!(
            "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
            team_id,
            teammate,
        )
        .execute(&mut *tx)
        .await?;
    }

    // --- Project ---
    let project_id = sqlx::query_scalar!(
        r#"
        INSERT INTO projects (team_id, name, slug, description, github_repo)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
        team_id,
        seed.project.name,
        format!("pettracker-{slug_suffix}"),
        seed.project.description,
        seed.project.github_repo,
    )
    .fetch_one(&mut *tx)
    .await?;

    // Project membership for everyone so teammates are assignable and the
    // participant (also Owner) sees the project.
    for member in std::iter::once(participant_id).chain(seed::TEAMMATES) {
        sqlx::query!(
            "INSERT INTO project_memberships (project_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            project_id,
            member,
        )
        .execute(&mut *tx)
        .await?;
    }

    let now = Utc::now();

    // --- Columns ---
    let mut column_map: HashMap<String, Uuid> = HashMap::new();
    for col in &seed.columns {
        let id = sqlx::query_scalar!(
            "INSERT INTO columns (project_id, name, position) VALUES ($1, $2, $3) RETURNING id",
            project_id,
            col.name,
            col.position,
        )
        .fetch_one(&mut *tx)
        .await?;
        column_map.insert(col.id.clone(), id);
    }

    // Wire the PR-driven board automation to the seeded columns. Without this the
    // project has no idea where "done" is, so approving-and-merging a pull request
    // in the review view would record the merge and move nothing. The seed has no
    // Review column, so `review_column_id` stays null — that mapping only drives
    // the create-a-PR-on-move prompt, which study mode locks out anyway.
    sqlx::query!(
        "UPDATE projects SET in_progress_column_id = $1, done_column_id = $2 WHERE id = $3",
        column_map.get("col_inprogress").copied(),
        column_map.get("col_done").copied(),
        project_id,
    )
    .execute(&mut *tx)
    .await?;

    // --- Labels ---
    let mut label_map: HashMap<String, Uuid> = HashMap::new();
    for label in &seed.labels {
        let id = sqlx::query_scalar!(
            "INSERT INTO labels (project_id, name, color) VALUES ($1, $2, $3) RETURNING id",
            project_id,
            label.name,
            label.color,
        )
        .fetch_one(&mut *tx)
        .await?;
        label_map.insert(label.id.clone(), id);
    }

    // --- Cards (tasks) ---
    let mut card_map: HashMap<String, Uuid> = HashMap::new();
    for card in &seed.cards {
        let column_id = column_map.get(&card.column_id).copied().ok_or_else(|| {
            ApiError::Internal(format!(
                "Seed card references unknown column {}",
                card.column_id
            ))
        })?;
        let assignee = card.assignee_id.as_deref().and_then(seed::teammate_uuid);
        let reporter = assignee.unwrap_or(seed::DEFAULT_AUTHOR);
        let priority = seed::priority_value(&card.priority);
        let created_at = offset_to_timestamp(now, card.created_offset);

        let task_id = sqlx::query_scalar!(
            r#"
            INSERT INTO tasks
              (project_id, column_id, title, description, assignee_id, reporter_id,
               priority, position, github_issue_id, github_pr_id, github_repo, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7::priority, $8, $9, $10, $11, $12)
            RETURNING id
            "#,
            project_id,
            column_id,
            card.title,
            card.description,
            assignee,
            reporter,
            priority as _,
            card.position,
            card.github_issue,
            card.github_pr,
            // Only set github_repo when a link exists so TaskDetail shows the
            // badge (and Unlink) rather than the blank link form.
            if card.github_issue.is_some() || card.github_pr.is_some() {
                seed.project.github_repo.as_deref()
            } else {
                None
            },
            created_at,
        )
        .fetch_one(&mut *tx)
        .await?;

        for label_sym in &card.labels {
            if let Some(label_id) = label_map.get(label_sym) {
                sqlx::query!(
                    "INSERT INTO task_labels (task_id, label_id) VALUES ($1, $2)",
                    task_id,
                    label_id,
                )
                .execute(&mut *tx)
                .await?;
            }
        }
        card_map.insert(card.id.clone(), task_id);
    }

    // --- Channels (+ memberships) ---
    let mut channel_map: HashMap<String, Uuid> = HashMap::new();
    for channel in &seed.channels {
        // project_scoped channels belong to the project; others are team-wide.
        let chan_project = channel.project_scoped.then_some(project_id);
        let channel_id = sqlx::query_scalar!(
            r#"
            INSERT INTO channels (team_id, project_id, name, created_by)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
            team_id,
            chan_project,
            channel.name,
            seed::DEFAULT_AUTHOR,
        )
        .fetch_one(&mut *tx)
        .await?;

        for member in std::iter::once(participant_id).chain(seed::TEAMMATES) {
            sqlx::query!(
                "INSERT INTO channel_memberships (channel_id, user_id) VALUES ($1, $2)",
                channel_id,
                member,
            )
            .execute(&mut *tx)
            .await?;
        }
        channel_map.insert(channel.id.clone(), channel_id);
    }

    // --- Messages (rewrite [[card:<fixture-id>]] -> [[card:<real-uuid>]]) ---
    for msg in &seed.messages {
        let channel_id = channel_map.get(&msg.channel_id).copied().ok_or_else(|| {
            ApiError::Internal(format!(
                "Seed message references unknown channel {}",
                msg.channel_id
            ))
        })?;
        let sender_id = seed::teammate_uuid(&msg.sender_id).ok_or_else(|| {
            ApiError::Internal(format!(
                "Seed message references unknown sender {}",
                msg.sender_id
            ))
        })?;
        let content = rewrite_card_refs(&msg.content, &card_map);
        let created_at = offset_to_timestamp(now, msg.created_offset);

        sqlx::query!(
            r#"
            INSERT INTO messages (channel_id, sender_id, content, created_at)
            VALUES ($1, $2, $3, $4)
            "#,
            channel_id,
            sender_id,
            content,
            created_at,
        )
        .execute(&mut *tx)
        .await?;
    }

    // --- Participant record ---
    // If the participant consented before provisioning (the normal flow),
    // record it now and advance status; otherwise leave the default.
    let (consent_at, status) = match consent_version {
        Some(_) => (Some(Utc::now()), "consented"),
        None => (None, "provisioned"),
    };
    let study_participant_id = sqlx::query_scalar!(
        r#"
        INSERT INTO study_participants
          (participant_code, user_id, team_id, project_id, agent_condition,
           consent_version, consent_given_at, status,
           participant_email, institution)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
        code,
        participant_id,
        team_id,
        project_id,
        condition,
        consent_version,
        consent_at,
        status,
        participant_email,
        institution,
    )
    .fetch_one(&mut *tx)
    .await?;

    // Spend the (already-locked) access code now that the participant exists.
    // Linked to the study_participants row (which the FK references), not the
    // throwaway user account.
    if let Some(code) = access_code {
        access_code::mark_used(&mut tx, code, study_participant_id).await?;
    }

    tx.commit().await?;

    // Session tokens (after commit so we never leave dangling refresh rows).
    let access_token = auth_helpers::generate_access_token(participant_id, "member", jwt_secret)?;
    let refresh_token = auth_helpers::create_refresh_token(pool, participant_id).await?;

    Ok(ProvisionResult {
        participant_code: code,
        agent_condition: condition,
        user_id: participant_id,
        team_id,
        project_id,
        access_token,
        refresh_token,
        resumed: false,
        resume_stage: None,
        resume_task_index: 0,
        baseline_tlx_done: false,
    })
}

/// Resume an existing participant by re-issuing a session for their account.
/// Used when a participant re-enters their (already-spent) access code, e.g. on
/// a new device or after clearing their browser. Creates nothing new — it hands
/// back the saved stage + task index so the frontend picks up where they left
/// off. The participant's work was never lost (it lives in their DB project).
pub async fn resume_participant(
    pool: &PgPool,
    jwt_secret: &str,
    user_id: Uuid,
) -> Result<ProvisionResult, ApiError> {
    let p = sqlx::query!(
        r#"SELECT participant_code, agent_condition, team_id, project_id,
                  resume_stage, resume_task_index,
                  EXISTS (
                      SELECT 1 FROM study_survey_responses r
                      WHERE r.participant_id = study_participants.id
                        AND right(r.item_id, 9) = '_baseline'
                  ) AS "baseline_tlx_done!"
           FROM study_participants WHERE user_id = $1"#,
        user_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::Internal("Access code has no participant to resume".into()))?;

    let access_token = auth_helpers::generate_access_token(user_id, "member", jwt_secret)?;
    let refresh_token = auth_helpers::create_refresh_token(pool, user_id).await?;

    Ok(ProvisionResult {
        participant_code: p.participant_code,
        agent_condition: p.agent_condition,
        user_id,
        team_id: p.team_id,
        project_id: p.project_id,
        access_token,
        refresh_token,
        resumed: true,
        resume_stage: p.resume_stage,
        resume_task_index: p.resume_task_index,
        baseline_tlx_done: p.baseline_tlx_done,
    })
}

/// Persist a participant's current place in the flow so they can resume on
/// another device. `stage` "done" marks completion (idempotent on completed_at).
pub async fn update_progress(
    pool: &PgPool,
    user_id: Uuid,
    stage: &str,
    task_index: i32,
) -> Result<(), ApiError> {
    let status = if stage == "done" {
        "completed"
    } else {
        "in_progress"
    };
    sqlx::query!(
        r#"UPDATE study_participants
           SET resume_stage = $2,
               resume_task_index = $3,
               status = $4,
               completed_at = CASE
                   WHEN $2 = 'done' THEN COALESCE(completed_at, NOW())
                   ELSE completed_at
               END
           WHERE user_id = $1"#,
        user_id,
        stage,
        task_index,
        status,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Convert a fixture minute-offset (negative = past) to an absolute timestamp.
fn offset_to_timestamp(now: DateTime<Utc>, offset_minutes: i64) -> DateTime<Utc> {
    now + Duration::minutes(offset_minutes)
}

/// Rewrite `[[card:<fixture-id>]]` tokens to point at the cloned task UUIDs so
/// chat cross-references resolve within the participant's isolated project.
fn rewrite_card_refs(content: &str, card_map: &HashMap<String, Uuid>) -> String {
    let mut out = content.to_string();
    for (fixture_id, task_id) in card_map {
        out = out.replace(
            &format!("[[card:{fixture_id}]]"),
            &format!("[[card:{task_id}]]"),
        );
    }
    out
}

/// Advisory-lock key serialising condition assignment. Arbitrary but fixed —
/// the ASCII of "WELDSTUD", so a stray lock is identifiable in `pg_locks`.
const CONDITION_LOCK_KEY: i64 = 0x5745_4C44_5354_5544;

/// Balanced *randomized* between-subjects assignment: pick the condition with
/// the fewest participants so far, breaking ties **at random**. With three
/// equal-target cells this is permuted-block randomization (block size 3) — each
/// run of 3 participants gets one of each condition, but the order within a
/// block is random. This keeps cell sizes even (which matters for a small-N
/// study) while avoiding the deterministic C1→C2→C3 cycle a fixed tie-break
/// produced, which would otherwise confound condition with enrollment order.
async fn pick_agent_condition(conn: &mut sqlx::PgConnection) -> Result<String, ApiError> {
    // Minimisation reads the cell counts and then writes one more row, so two
    // provisions racing each other would both see the pre-insert counts and both
    // claim the same least-filled cell — the imbalance minimisation exists to
    // prevent. Serialise it: the lock is held until the transaction ends, so the
    // second provision reads counts that already include the first.
    sqlx::query!("SELECT pg_advisory_xact_lock($1)", CONDITION_LOCK_KEY)
        .execute(&mut *conn)
        .await?;

    let rows = sqlx::query!(
        r#"SELECT agent_condition, COUNT(*) as "count!" FROM study_participants GROUP BY agent_condition"#
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut counts: HashMap<&str, i64> = HashMap::from([("C1", 0), ("C2", 0), ("C3", 0)]);
    for row in &rows {
        if let Some(c) = counts.get_mut(row.agent_condition.as_str()) {
            *c = row.count;
        }
    }

    let min = ["C1", "C2", "C3"]
        .into_iter()
        .map(|c| counts[c])
        .min()
        .unwrap_or(0);
    let tied: Vec<&str> = ["C1", "C2", "C3"]
        .into_iter()
        .filter(|c| counts[c] == min)
        .collect();
    let chosen = tied
        .get(rand::thread_rng().gen_range(0..tied.len()))
        .copied()
        .unwrap_or("C1");
    Ok(chosen.to_string())
}
