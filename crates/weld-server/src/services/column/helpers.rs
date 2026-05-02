use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::task::Column;

/// Smallest gap allowed between two `position` values before the API forces
/// the client to refetch and rebalance. Matches the threshold called out in
/// the board-API issue.
pub const POSITION_PRECISION_FLOOR: f64 = 1e-6;

pub async fn fetch_column(pool: &PgPool, column_id: Uuid) -> Result<Column, ApiError> {
    sqlx::query_as!(
        Column,
        r#"
        SELECT id, project_id, name, position, wip_limit, created_at, updated_at
        FROM columns
        WHERE id = $1
        "#,
        column_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Column not found".into()))
}

/// Compute the `position` for a new column appended to the end of a project.
pub async fn next_column_position(pool: &PgPool, project_id: Uuid) -> Result<f64, ApiError> {
    let max: Option<f64> = sqlx::query_scalar!(
        "SELECT MAX(position) FROM columns WHERE project_id = $1",
        project_id
    )
    .fetch_one(pool)
    .await?;
    Ok(max.map(|m| m + 1.0).unwrap_or(1.0))
}
