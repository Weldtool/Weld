use crate::errors::ApiError;
use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
pub async fn me(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<UserResponse>, ApiError> {
    let user = sqlx::query!(
        r#"
        SELECT id, email, display_name, avatar_url, role::text as "role!", email_verified, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
        auth.user_id
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(UserResponse {
        id: user.id,
        email: user.email,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
        role: user.role,
        email_verified: user.email_verified,
        created_at: user.created_at,
        updated_at: user.updated_at,
    }))
}
