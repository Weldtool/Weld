use crate::errors::ApiError;
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::task::Label;

pub async fn fetch_label(pool: &PgPool, label_id: Uuid) -> Result<Label, ApiError> {
    sqlx::query_as!(
        Label,
        r#"
        SELECT id, project_id, name, color, created_at
        FROM labels
        WHERE id = $1
        "#,
        label_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Label not found".into()))
}
