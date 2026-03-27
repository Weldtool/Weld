use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Router, routing::get, routing::post};
use serde_json::json;
mod auth;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/auth/register", post(auth::register))
}

async fn health_check(State(_state): State<AppState>) -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}
