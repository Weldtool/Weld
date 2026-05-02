use super::helpers::fetch_column;
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub async fn delete_column(
    pool: &PgPool,
    column_id: Uuid,
    caller_id: Uuid,
) -> Result<(), ApiError> {
    let column = fetch_column(pool, column_id).await?;
    let project = fetch_active_project(pool, column.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Admin).await?;

    let active_tasks: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM tasks WHERE column_id = $1 AND deleted_at IS NULL",
        column_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    if active_tasks > 0 {
        return Err(ApiError::Conflict(
            "Column still has active tasks; move or delete them first".into(),
        ));
    }

    sqlx::query!("DELETE FROM columns WHERE id = $1", column_id)
        .execute(pool)
        .await?;

    Ok(())
}
