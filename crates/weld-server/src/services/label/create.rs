use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::task::Label;

pub async fn create_label(
    pool: &PgPool,
    project_id: Uuid,
    caller_id: Uuid,
    name: &str,
    color: &str,
) -> Result<Label, ApiError> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err(ApiError::Validation("Label name cannot be empty".into()));
    }

    let project = fetch_active_project(pool, project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    let label = sqlx::query_as!(
        Label,
        r#"
        INSERT INTO labels (project_id, name, color)
        VALUES ($1, $2, $3)
        RETURNING id, project_id, name, color, created_at
        "#,
        project_id,
        trimmed_name,
        color
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("labels_project_id_name_key") => {
            ApiError::Conflict("Label with this name already exists".into())
        }
        _ => ApiError::from(e),
    })?;

    Ok(label)
}
