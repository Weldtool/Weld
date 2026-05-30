//! The context-scoped study agent (CLAUDE_AGENT_BUILD.md). The Context Gate
//! (`gate`) enforces per-condition scoping; `tools` defines the double-gated
//! capabilities; `openai` is the pinned model client; `run` orchestrates a
//! single agent turn and records the study-critical telemetry.

pub mod gate;
pub mod openai;
pub mod run;
pub mod tools;

use uuid::Uuid;

use gate::AgentCondition;

/// Everything the agent needs about the participant it's acting for. Built once
/// per request from the authenticated user; carries the isolated team/project
/// the Gate and tools operate on.
pub struct ParticipantCtx {
    pub participant_id: Uuid,
    pub user_id: Uuid,
    pub team_id: Uuid,
    pub project_id: Uuid,
    pub condition: AgentCondition,
}

/// Load the participant context for an authenticated study user, or `None` if
/// the user isn't a study participant.
pub async fn load_participant(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<Option<ParticipantCtx>, crate::errors::ApiError> {
    let row = sqlx::query!(
        r#"
        SELECT id, team_id, project_id, agent_condition
        FROM study_participants
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(None),
        Some(r) => Ok(Some(ParticipantCtx {
            participant_id: r.id,
            user_id,
            team_id: r.team_id,
            project_id: r.project_id,
            condition: AgentCondition::parse(&r.agent_condition)?,
        })),
    }
}
