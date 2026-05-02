use super::fetch_label;
use crate::errors::ApiError;
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::task::Label;

#[derive(Default)]
pub struct UpdateLabelInput {
    pub name: Option<String>,
    pub color: Option<String>,
}

pub async fn update_label(
    pool: &PgPool,
    label_id: Uuid,
    caller_id: Uuid,
    input: UpdateLabelInput,
) -> Result<Label, ApiError> {
    let label = fetch_label(pool, label_id).await?;
    let project = fetch_active_project(pool, label.project_id).await?;
    require_project_team_role(pool, &project, caller_id, TeamRole::Member).await?;

    if let Some(name) = &input.name
        && name.trim().is_empty()
    {
        return Err(ApiError::Validation("Label name cannot be empty".into()));
    }

    let new_name = input
        .name
        .as_ref()
        .map(|n| n.trim().to_string())
        .unwrap_or_else(|| label.name.clone());
    let new_color = input.color.unwrap_or_else(|| label.color.clone());

    let updated = sqlx::query_as!(
        Label,
        r#"
        UPDATE labels
        SET name = $1, color = $2
        WHERE id = $3
        RETURNING id, project_id, name, color, created_at
        "#,
        new_name,
        new_color,
        label_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("labels_project_id_name_key") => {
            ApiError::Conflict("Label with this name already exists".into())
        }
        _ => ApiError::from(e),
    })?;

    Ok(updated)
}
