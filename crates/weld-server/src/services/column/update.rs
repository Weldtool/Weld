use super::helpers::fetch_column;
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::task::Column;

#[derive(Default)]
pub struct UpdateColumnInput {
    pub name: Option<String>,
    pub position: Option<f64>,
    pub wip_limit: Option<Option<i32>>,
}

pub async fn update_column(
    pool: &PgPool,
    column_id: Uuid,
    caller_id: Uuid,
    input: UpdateColumnInput,
) -> Result<Column, ApiError> {
    let column = fetch_column(pool, column_id).await?;
    let project = fetch_active_project(pool, column.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    if let Some(name) = &input.name
        && name.trim().is_empty()
    {
        return Err(ApiError::Validation("Column name cannot be empty".into()));
    }

    let new_name = input
        .name
        .as_ref()
        .map(|n| n.trim().to_string())
        .unwrap_or_else(|| column.name.clone());
    let new_position = input.position.unwrap_or(column.position);
    let new_wip = match &input.wip_limit {
        Some(v) => *v,
        None => column.wip_limit,
    };

    let updated = sqlx::query_as!(
        Column,
        r#"
        UPDATE columns
        SET name = $1, position = $2, wip_limit = $3, updated_at = NOW()
        WHERE id = $4
        RETURNING id, project_id, name, position, wip_limit, created_at, updated_at
        "#,
        new_name,
        new_position,
        new_wip,
        column_id
    )
    .fetch_one(pool)
    .await?;

    Ok(updated)
}
