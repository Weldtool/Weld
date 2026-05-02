use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::label::{self, UpdateLabelInput};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use weld_core::models::task::Label;

#[derive(Debug, Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLabelRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LabelResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
}

impl From<Label> for LabelResponse {
    fn from(l: Label) -> Self {
        Self {
            id: l.id,
            project_id: l.project_id,
            name: l.name,
            color: l.color,
            created_at: l.created_at,
        }
    }
}

pub async fn create(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateLabelRequest>,
) -> Result<(StatusCode, Json<LabelResponse>), ApiError> {
    let label = label::create_label(
        &state.db,
        project_id,
        auth.user_id,
        &input.name,
        &input.color,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(label.into())))
}

pub async fn update(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateLabelRequest>,
) -> Result<Json<LabelResponse>, ApiError> {
    let label = label::update_label(
        &state.db,
        id,
        auth.user_id,
        UpdateLabelInput {
            name: input.name,
            color: input.color,
        },
    )
    .await?;
    Ok(Json(label.into()))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    label::delete_label(&state.db, id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
