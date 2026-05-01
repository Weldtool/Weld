use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, post};
use axum::{Router, routing};
use serde_json::json;
mod auth;
mod projects;
mod teams;
mod users;
pub fn router() -> Router<AppState> {
    let public = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/refresh", post(auth::refresh_token));

    let protected = Router::new()
        .route("/api/v1/users/me", get(users::me))
        .route("/api/v1/teams", get(teams::list).post(teams::create))
        .route("/api/v1/teams/{team_id}/members", post(teams::add_member))
        .route(
            "/api/v1/teams/{team_id}/projects",
            routing::get(projects::list_for_team).post(projects::create),
        )
        .route("/api/v1/projects/{id}", get(projects::get_by_id))
        .route("/api/v1/projects/{id}", patch(projects::update))
        .route("/api/v1/projects/{id}", delete(projects::delete));

    public.merge(protected)
}

async fn health_check(State(_state): State<AppState>) -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}
