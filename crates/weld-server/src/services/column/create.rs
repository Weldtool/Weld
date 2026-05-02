use super::helpers::next_column_position;
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::task::Column;

pub async fn create_column(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
    name: &str,
    position: Option<f64>,
    wip_limit: Option<i32>,
) -> Result<Column, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Validation("Column name cannot be empty".into()));
    }

    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Admin).await?;

    let pos = match position {
        Some(p) => p,
        None => next_column_position(pool, project_id).await?,
    };

    let column = sqlx::query_as!(
        Column,
        r#"
        INSERT INTO columns (project_id, name, position, wip_limit)
        VALUES ($1, $2, $3, $4)
        RETURNING id, project_id, name, position, wip_limit, created_at, updated_at
        "#,
        project_id,
        trimmed,
        pos,
        wip_limit
    )
    .fetch_one(pool)
    .await?;

    Ok(column)
}
