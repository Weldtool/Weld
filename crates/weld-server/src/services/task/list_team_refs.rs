use crate::errors::ApiError;
use crate::services::team::helpers::require_team_role;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

/// Lightweight task reference used for `#` autocomplete and rendering chat
/// references. Scoped to a team's active projects.
#[derive(Debug, Serialize)]
pub struct TaskRef {
    pub id: Uuid,
    pub short_id: i64,
    pub title: String,
    pub project_id: Uuid,
    /// Number of the GitHub PR linked to this task, if any. Lets the client
    /// resolve `PR#<number>` chat references to the task they're linked to.
    pub github_pr_id: Option<i64>,
}

/// List referenceable tasks across a team's active projects. Caller must be a
/// member of the team (Viewer+).
pub async fn list_team_task_refs(
    pool: &PgPool,
    team_id: Uuid,
    caller_id: Uuid,
) -> Result<Vec<TaskRef>, ApiError> {
    require_team_role(pool, team_id, caller_id, TeamRole::Viewer).await?;

    let refs = sqlx::query_as!(
        TaskRef,
        r#"
        SELECT t.id, t.short_id, t.title, t.project_id, t.github_pr_id
        FROM tasks t
        JOIN projects p ON p.id = t.project_id
        WHERE p.team_id = $1
          AND p.deleted_at IS NULL
          AND t.deleted_at IS NULL
        ORDER BY t.short_id DESC
        "#,
        team_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(refs)
}
