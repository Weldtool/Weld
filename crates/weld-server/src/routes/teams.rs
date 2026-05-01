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
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
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
                created_at: t.created_at,
            })
            .collect(),
    ))
}

pub async fn add_member(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    Json(input): Json<AddMemberRequest>,
) -> Result<StatusCode, ApiError> {
    team::add_team_member(&state.db, team_id, auth.user_id, input.user_id, input.role).await?;
    Ok(StatusCode::CREATED)
}
