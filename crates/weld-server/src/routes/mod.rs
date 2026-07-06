use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, post};
use axum::{Router, routing};
use serde_json::json;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};
mod auth;
mod board;
pub mod chat;
pub mod columns;
mod github;
pub mod labels;
mod projects;
mod study;
pub mod tasks;
mod teams;
mod users;
mod ws;
pub fn router() -> Router<AppState> {
    let public = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/config", get(public_config))
        // Study participant provisioning — public (no account yet), but the
        // handler 404s unless STUDY_MODE is on.
        .route("/api/v1/study/provision", post(study::provision))
        // Study data export — guarded by STUDY_EXPORT_TOKEN, not participant JWT,
        // so it lives here (public router) and 404s unless STUDY_MODE + token set.
        .route("/api/v1/study/export", get(study::export_data))
        // Aggregate summary for the researcher dashboard (same token guard).
        .route("/api/v1/study/summary", get(study::get_summary))
        // Mint single-use access codes — same researcher token as export.
        .route("/api/v1/study/codes", post(study::generate_codes))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/refresh", post(auth::refresh_token))
        .route("/api/v1/ws", get(ws::ws_upgrade))
        // GitHub callbacks — public because GitHub redirects the browser here
        .route("/api/v1/github/setup", get(github::setup_callback))
        .route("/api/v1/github/oauth/callback", get(github::oauth_callback))
        .route("/api/v1/github/webhook", post(github::webhook));

    let protected = Router::new()
        .route("/api/v1/users/me", get(users::me))
        // Study telemetry ingest (auth'd participant; 404s unless STUDY_MODE).
        .route("/api/v1/study/events", post(study::ingest_events))
        .route("/api/v1/study/progress", post(study::save_progress))
        .route("/api/v1/study/survey", post(study::submit_survey))
        .route(
            "/api/v1/study/interim-survey",
            post(study::submit_interim_survey),
        )
        .route("/api/v1/study/agent", post(study::agent_turn))
        .route("/api/v1/teams", get(teams::list).post(teams::create))
        .route(
            "/api/v1/teams/{team_id}",
            patch(teams::update).delete(teams::delete),
        )
        .route(
            "/api/v1/teams/{team_id}/members",
            get(teams::list_members).post(teams::add_member),
        )
        .route(
            "/api/v1/teams/{team_id}/members/{user_id}",
            axum::routing::delete(teams::remove_member),
        )
        .route(
            "/api/v1/teams/{team_id}/projects",
            routing::get(projects::list_for_team).post(projects::create),
        )
        .route("/api/v1/projects/{id}", get(projects::get_by_id))
        .route("/api/v1/projects/{id}", patch(projects::update))
        .route("/api/v1/projects/{id}", delete(projects::delete))
        .route(
            "/api/v1/projects/{id}/members",
            get(projects::list_members).post(projects::add_member),
        )
        .route(
            "/api/v1/projects/{id}/members/{user_id}",
            delete(projects::remove_member),
        )
        .route(
            "/api/v1/teams/{team_id}/task-refs",
            get(tasks::list_team_task_refs),
        )
        .route("/api/v1/projects/{id}/board", get(board::get_board))
        .route("/api/v1/projects/{id}/columns", post(columns::create))
        .route("/api/v1/columns/{id}", patch(columns::update))
        .route("/api/v1/columns/{id}", delete(columns::delete))
        .route("/api/v1/projects/{id}/labels", post(labels::create))
        .route("/api/v1/labels/{id}", patch(labels::update))
        .route("/api/v1/labels/{id}", delete(labels::delete))
        .route("/api/v1/projects/{id}/tasks", post(tasks::create))
        .route("/api/v1/tasks/{id}", get(tasks::get_by_id))
        .route("/api/v1/tasks/{id}", patch(tasks::update))
        .route("/api/v1/tasks/{id}", delete(tasks::delete))
        .route("/api/v1/tasks/{id}/move", post(tasks::move_task))
        .route(
            "/api/v1/tasks/{id}/link",
            post(tasks::link_task).delete(tasks::unlink_task),
        )
        .route(
            "/api/v1/tasks/{id}/push-to-github",
            post(tasks::push_task_to_github),
        )
        .route("/api/v1/tasks/{id}/create-pr", post(tasks::create_pr))
        .route(
            "/api/v1/projects/{id}/github/branches",
            get(github::list_project_branches),
        )
        .route(
            "/api/v1/projects/{id}/github/pr-template",
            get(github::get_pr_template),
        )
        .route(
            "/api/v1/projects/{id}/github/issue-templates",
            get(github::list_issue_templates),
        )
        .route(
            "/api/v1/projects/{id}/github/issues",
            get(github::list_project_issues),
        )
        .route(
            "/api/v1/projects/{id}/github/issues/{number}",
            get(github::get_project_issue),
        )
        .route(
            "/api/v1/projects/{id}/github/prs",
            get(github::list_project_prs),
        )
        .route(
            "/api/v1/projects/{id}/github/labels/sync",
            post(github::sync_project_labels),
        )
        .route(
            "/api/v1/teams/{team_id}/channels",
            get(chat::list_channels).post(chat::create_channel),
        )
        .route(
            "/api/v1/channels/{id}",
            get(chat::get_channel)
                .patch(chat::update_channel)
                .delete(chat::archive_channel),
        )
        .route("/api/v1/channels/{id}/members", post(chat::add_member))
        .route(
            "/api/v1/channels/{channel_id}/members/{user_id}",
            delete(chat::remove_member),
        )
        .route("/api/v1/channels/{id}/read", post(chat::mark_read))
        .route(
            "/api/v1/channels/{id}/messages",
            get(chat::list_messages).post(chat::post_message),
        )
        .route("/api/v1/messages/{id}/thread", get(chat::get_thread))
        .route(
            "/api/v1/messages/{id}",
            patch(chat::edit_message).delete(chat::delete_message),
        )
        // GitHub — protected
        .route(
            "/api/v1/teams/{team_id}/github/install-url",
            get(github::get_install_url),
        )
        .route("/api/v1/github/oauth/url", get(github::get_oauth_url))
        .route(
            "/api/v1/teams/{team_id}/github",
            get(github::get_connection).delete(github::disconnect),
        )
        .route(
            "/api/v1/teams/{team_id}/github/repos",
            get(github::list_installation_repos),
        );

    let frontend_origin =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let cors = CorsLayer::new()
        .allow_origin(
            frontend_origin
                .parse::<HeaderValue>()
                .expect("FRONTEND_URL must be a valid HTTP origin"),
        )
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request())
        .allow_credentials(true);

    public.merge(protected).layer(cors)
}

async fn health_check(State(_state): State<AppState>) -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

/// Public, unauthenticated runtime config the frontend reads on load to decide
/// which behavior to render. Intentionally exposes only non-sensitive flags.
async fn public_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "study_mode": state.study_mode,
        "study_require_access_code": state.study_require_access_code,
    }))
}
