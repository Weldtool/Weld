use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Router, routing::get};
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health_check))
}

async fn health_check(State(_state): State<AppState>) -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}
