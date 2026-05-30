use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::realtime::broadcast::publish_project_event;
use crate::services::github::sync;
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

#[derive(Debug, Deserialize)]
pub struct LinkTaskRequest {
    pub repo: String,
    pub issue_number: Option<i64>,
    pub pr_number: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePrRequest {
    pub head: String,
    pub base: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Deserialize)]
pub struct PushToGithubRequest {
    pub title: Option<String>,
    pub body: Option<String>,
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
    pub short_id: i64,
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
    pub github_repo: Option<String>,
    pub labels: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TaskWithLabels> for TaskResponse {
    fn from(t: TaskWithLabels) -> Self {
        let TaskWithLabels { task, labels } = t;
        Self {
            id: task.id,
            short_id: task.short_id,
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
            github_repo: task.github_repo,
            labels,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

pub async fn list_team_task_refs(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<task::TaskRef>>, ApiError> {
    let refs = task::list_team_task_refs(&state.db, team_id, auth.user_id).await?;
    Ok(Json(refs))
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
    let resp: TaskResponse = task.into();
    publish_project_event(&state.hub, project_id, "task.created", &resp);
    Ok((StatusCode::CREATED, Json(resp)))
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
    let resp: TaskResponse = task.into();
    publish_project_event(&state.hub, resp.project_id, "task.updated", &resp);
    Ok(Json(resp))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let existing = task::get_task(&state.db, id, auth.user_id).await?;
    let project_id = existing.task.project_id;
    task::delete_task(&state.db, id, auth.user_id).await?;
    publish_project_event(
        &state.hub,
        project_id,
        "task.deleted",
        &serde_json::json!({ "task_id": id, "project_id": project_id }),
    );
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
    let resp: TaskResponse = task.into();
    publish_project_event(&state.hub, resp.project_id, "task.moved", &resp);

    Ok(Json(resp))
}

pub async fn link_task(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<LinkTaskRequest>,
) -> Result<Json<TaskResponse>, ApiError> {
    if input.issue_number.is_none() && input.pr_number.is_none() {
        return Err(ApiError::Validation(
            "Provide issue_number or pr_number".into(),
        ));
    }
    let task = sync::link_task(
        &state.db,
        id,
        auth.user_id,
        &input.repo,
        input.issue_number,
        input.pr_number,
    )
    .await?;
    let resp: TaskResponse = task.into();
    publish_project_event(&state.hub, resp.project_id, "task.updated", &resp);
    Ok(Json(resp))
}

pub async fn unlink_task(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskResponse>, ApiError> {
    let task = sync::unlink_task(&state.db, id, auth.user_id).await?;
    let resp: TaskResponse = task.into();
    publish_project_event(&state.hub, resp.project_id, "task.updated", &resp);
    Ok(Json(resp))
}

pub async fn push_task_to_github(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    body: Option<Json<PushToGithubRequest>>,
) -> Result<Json<TaskResponse>, ApiError> {
    if state.study_mode {
        return Err(ApiError::Forbidden(
            "GitHub writes are disabled in study mode".into(),
        ));
    }
    let gh_cfg = state.github.as_ref().ok_or_else(|| {
        ApiError::Validation("GitHub integration is not configured on the server".into())
    })?;
    let (title, issue_body) = match body {
        Some(Json(input)) => (input.title, input.body),
        None => (None, None),
    };
    let task =
        sync::push_task_to_github(&state.db, gh_cfg, id, auth.user_id, title, issue_body).await?;
    let resp: TaskResponse = task.into();
    publish_project_event(&state.hub, resp.project_id, "task.updated", &resp);
    Ok(Json(resp))
}

pub async fn create_pr(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<CreatePrRequest>,
) -> Result<Json<TaskResponse>, ApiError> {
    if state.study_mode {
        return Err(ApiError::Forbidden(
            "GitHub writes are disabled in study mode".into(),
        ));
    }
    let gh_cfg = state.github.as_ref().ok_or_else(|| {
        ApiError::Validation("GitHub integration is not configured on the server".into())
    })?;
    let task = sync::create_pr_for_task(
        &state.db,
        gh_cfg,
        id,
        auth.user_id,
        sync::CreatePrInput {
            head: input.head,
            base: input.base,
            title: input.title,
            body: input.body,
            draft: input.draft,
        },
    )
    .await?;
    let resp: TaskResponse = task.into();
    publish_project_event(&state.hub, resp.project_id, "task.updated", &resp);
    Ok(Json(resp))
}
