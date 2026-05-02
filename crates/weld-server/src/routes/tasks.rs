use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::task::{self, CreateTaskInput, TaskWithLabels, UpdateTaskInput};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use weld_core::models::enums::Priority;

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub column_id: Uuid,
    pub assignee_id: Option<Uuid>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<Uuid>>,
    pub due_date: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub assignee_id: Option<Option<Uuid>>,
    pub priority: Option<Priority>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub due_date: Option<Option<DateTime<Utc>>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub estimate_hours: Option<Option<f32>>,
    pub labels: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct MoveTaskRequest {
    pub column_id: Uuid,
    pub position: f64,
}

fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub column_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub reporter_id: Uuid,
    pub priority: Priority,
    pub position: f64,
    pub due_date: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f32>,
    pub github_issue_id: Option<i64>,
    pub github_pr_id: Option<i64>,
    pub labels: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TaskWithLabels> for TaskResponse {
    fn from(t: TaskWithLabels) -> Self {
        let TaskWithLabels { task, labels } = t;
        Self {
            id: task.id,
            project_id: task.project_id,
            column_id: task.column_id,
            title: task.title,
            description: task.description,
            assignee_id: task.assignee_id,
            reporter_id: task.reporter_id,
            priority: task.priority,
            position: task.position,
            due_date: task.due_date,
            estimate_hours: task.estimate_hours,
            github_issue_id: task.github_issue_id,
            github_pr_id: task.github_pr_id,
            labels,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

pub async fn create(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), ApiError> {
    let task = task::create_task(
        &state.db,
        project_id,
        auth.user_id,
        CreateTaskInput {
            title: input.title,
            description: input.description,
            column_id: input.column_id,
            assignee_id: input.assignee_id,
            priority: input.priority,
            labels: input.labels,
            due_date: input.due_date,
            estimate_hours: input.estimate_hours,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(task.into())))
}

pub async fn get_by_id(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskResponse>, ApiError> {
    let task = task::get_task(&state.db, id, auth.user_id).await?;
    Ok(Json(task.into()))
}

pub async fn update(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTaskRequest>,
) -> Result<Json<TaskResponse>, ApiError> {
    let task = task::update_task(
        &state.db,
        id,
        auth.user_id,
        UpdateTaskInput {
            title: input.title,
            description: input.description,
            assignee_id: input.assignee_id,
            priority: input.priority,
            due_date: input.due_date,
            estimate_hours: input.estimate_hours,
            labels: input.labels,
        },
    )
    .await?;
    Ok(Json(task.into()))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    task::delete_task(&state.db, id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn move_task(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<MoveTaskRequest>,
) -> Result<Json<TaskResponse>, ApiError> {
    let task =
        task::move_task(&state.db, id, auth.user_id, input.column_id, input.position).await?;
    Ok(Json(task.into()))
}
