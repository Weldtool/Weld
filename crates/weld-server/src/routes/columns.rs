use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::column::{self, UpdateColumnInput};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use weld_core::models::task::Column;

#[derive(Debug, Deserialize)]
pub struct CreateColumnRequest {
    pub name: String,
    pub position: Option<f64>,
    pub wip_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateColumnRequest {
    pub name: Option<String>,
    pub position: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub wip_limit: Option<Option<i32>>,
}

fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[derive(Debug, Serialize)]
pub struct ColumnResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub position: f64,
    pub wip_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Column> for ColumnResponse {
    fn from(c: Column) -> Self {
        Self {
            id: c.id,
            project_id: c.project_id,
            name: c.name,
            position: c.position,
            wip_limit: c.wip_limit,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

pub async fn create(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateColumnRequest>,
) -> Result<(StatusCode, Json<ColumnResponse>), ApiError> {
    let column = column::create_column(
        &state.db,
        project_id,
        auth.user_id,
        &input.name,
        input.position,
        input.wip_limit,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(column.into())))
}

pub async fn update(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateColumnRequest>,
) -> Result<Json<ColumnResponse>, ApiError> {
    let column = column::update_column(
        &state.db,
        id,
        auth.user_id,
        UpdateColumnInput {
            name: input.name,
            position: input.position,
            wip_limit: input.wip_limit,
        },
    )
    .await?;
    Ok(Json(column.into()))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    column::delete_column(&state.db, id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
