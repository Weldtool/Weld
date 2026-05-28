use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::project::{self, UpdateProjectInput};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use weld_core::models::project::Project;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProjectRequest {
    #[validate(length(min = 1, max = 100, message = "Project name must be 1-100 characters"))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub github_repo: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub done_column_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub in_progress_column_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub review_column_id: Option<Option<Uuid>>,
    pub auto_close_on_issue_closed: Option<bool>,
}

fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub team_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub github_repo: Option<String>,
    pub done_column_id: Option<Uuid>,
    pub in_progress_column_id: Option<Uuid>,
    pub review_column_id: Option<Uuid>,
    pub auto_close_on_issue_closed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            team_id: p.team_id,
            name: p.name,
            slug: p.slug,
            description: p.description,
            github_repo: p.github_repo,
            done_column_id: p.done_column_id,
            in_progress_column_id: p.in_progress_column_id,
            review_column_id: p.review_column_id,
            auto_close_on_issue_closed: p.auto_close_on_issue_closed,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

pub async fn create(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    Json(input): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ApiError> {
    input
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let project = project::create_project(
        &state.db,
        team_id,
        auth.user_id,
        &input.name,
        input.description.as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(project.into())))
}

pub async fn list_for_team(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<ProjectResponse>>, ApiError> {
    let projects = project::list_team_projects(&state.db, team_id, auth.user_id).await?;
    Ok(Json(projects.into_iter().map(Into::into).collect()))
}

pub async fn get_by_id(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let project = project::get_project(&state.db, id, auth.user_id).await?;
    Ok(Json(project.into()))
}

pub async fn update(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let project = project::update_project(
        &state.db,
        id,
        auth.user_id,
        UpdateProjectInput {
            name: input.name,
            description: input.description,
            github_repo: input.github_repo,
            done_column_id: input.done_column_id,
            in_progress_column_id: input.in_progress_column_id,
            review_column_id: input.review_column_id,
            auto_close_on_issue_closed: input.auto_close_on_issue_closed,
        },
    )
    .await?;
    Ok(Json(project.into()))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    project::delete_project(&state.db, id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct ProjectMemberResponse {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub team_role: weld_core::models::enums::TeamRole,
    pub added_at: DateTime<Utc>,
}

impl From<project::ProjectMember> for ProjectMemberResponse {
    fn from(m: project::ProjectMember) -> Self {
        Self {
            user_id: m.user_id,
            email: m.email,
            display_name: m.display_name,
            avatar_url: m.avatar_url,
            team_role: m.team_role,
            added_at: m.added_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AddProjectMemberRequest {
    /// One of `user_id` or `email` is required. `email` is matched
    /// case-insensitively against registered users.
    pub user_id: Option<Uuid>,
    pub email: Option<String>,
}

pub async fn list_members(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ProjectMemberResponse>>, ApiError> {
    let members = project::list_project_members(&state.db, id, auth.user_id).await?;
    Ok(Json(members.into_iter().map(Into::into).collect()))
}

pub async fn add_member(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<AddProjectMemberRequest>,
) -> Result<(StatusCode, Json<ProjectMemberResponse>), ApiError> {
    let new_member_id = match (input.user_id, input.email.as_deref()) {
        (Some(uid), _) => uid,
        (None, Some(email)) if !email.trim().is_empty() => {
            crate::services::team::find_user_id_by_email(&state.db, email.trim()).await?
        }
        _ => {
            return Err(ApiError::Validation(
                "Provide either user_id or email".into(),
            ));
        }
    };

    let member = project::add_project_member(&state.db, id, auth.user_id, new_member_id).await?;
    Ok((StatusCode::CREATED, Json(member.into())))
}

pub async fn remove_member(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    project::remove_project_member(&state.db, id, auth.user_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
