use super::fetch_label;
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

pub async fn delete_label(pool: &PgPool, label_id: Uuid, caller_id: Uuid) -> Result<(), ApiError> {
    let label = fetch_label(pool, label_id).await?;
    let project = fetch_active_project(pool, label.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    // task_labels rows cascade on label deletion via FK ON DELETE CASCADE.
    sqlx::query!("DELETE FROM labels WHERE id = $1", label_id)
        .execute(pool)
        .await?;
    Ok(())
}
