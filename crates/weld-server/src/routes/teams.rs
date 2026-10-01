use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::services::team;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use weld_core::models::enums::TeamRole;

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTeamRequest {
    #[validate(length(min = 1, max = 100, message = "Team name must be 1-100 characters"))]
    pub name: String,
}

pub async fn update(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    Json(input): Json<UpdateTeamRequest>,
) -> Result<Json<UserTeamResponse>, ApiError> {
    input
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let (updated, role) = team::update_team(&state.db, team_id, auth.user_id, &input.name).await?;
    Ok(Json(UserTeamResponse {
        id: updated.id,
        name: updated.name,
        slug: updated.slug,
        owner_id: updated.owner_id,
        role,
        is_personal: updated.is_personal,
        created_at: updated.created_at,
    }))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    team::delete_team(&state.db, team_id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTeamRequest {
    #[validate(length(min = 1, max = 100, message = "Team name must be 1-100 characters"))]
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TeamResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserTeamResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_id: Uuid,
    pub role: TeamRole,
    pub is_personal: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddMemberRequest {
    /// One of `user_id` or `email` is required. `email` is matched
    /// case-insensitively against registered users.
    pub user_id: Option<Uuid>,
    pub email: Option<String>,
    pub role: TeamRole,
}

pub async fn create(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateTeamRequest>,
) -> Result<(StatusCode, Json<TeamResponse>), ApiError> {
    input
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let created = team::create_team(&state.db, auth.user_id, &input.name).await?;
    Ok((
        StatusCode::CREATED,
        Json(TeamResponse {
            id: created.id,
            name: created.name,
            slug: created.slug,
            owner_id: created.owner_id,
            created_at: created.created_at,
        }),
    ))
}

pub async fn list(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserTeamResponse>>, ApiError> {
    let teams = team::list_user_teams(&state.db, auth.user_id).await?;
    Ok(Json(
        teams
            .into_iter()
            .map(|t| UserTeamResponse {
                id: t.id,
                name: t.name,
                slug: t.slug,
                owner_id: t.owner_id,
                role: t.role,
                is_personal: t.is_personal,
                created_at: t.created_at,
            })
            .collect(),
    ))
}

#[derive(Debug, Serialize)]
pub struct TeamMemberResponse {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: TeamRole,
    pub joined_at: DateTime<Utc>,
}

pub async fn list_members(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<TeamMemberResponse>>, ApiError> {
    let members = team::list_team_members(&state.db, team_id, auth.user_id).await?;
    Ok(Json(
        members
            .into_iter()
            .map(|m| TeamMemberResponse {
                user_id: m.user_id,
                email: m.email,
                display_name: m.display_name,
                avatar_url: m.avatar_url,
                role: m.role,
                joined_at: m.joined_at,
            })
            .collect(),
    ))
}

pub async fn add_member(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    Json(input): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<TeamMemberResponse>), ApiError> {
    let new_member_id = match (input.user_id, input.email.as_deref()) {
        (Some(id), _) => id,
        (None, Some(email)) if !email.trim().is_empty() => {
            team::find_user_id_by_email(&state.db, email.trim()).await?
        }
        _ => {
            return Err(ApiError::Validation(
                "Provide either user_id or email".into(),
            ));
        }
    };

    let member =
        team::add_team_member(&state.db, team_id, auth.user_id, new_member_id, input.role).await?;
    Ok((
        StatusCode::CREATED,
        Json(TeamMemberResponse {
            user_id: member.user_id,
            email: member.email,
            display_name: member.display_name,
            avatar_url: member.avatar_url,
            role: member.role,
            joined_at: member.joined_at,
        }),
    ))
}

pub async fn remove_member(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((team_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    team::remove_team_member(&state.db, team_id, auth.user_id, user_id).await?;
    // Stop streaming the team's rooms to any socket they still have open.
    super::ws::revalidate_user_rooms(&state, user_id).await;
    Ok(StatusCode::NO_CONTENT)
}
