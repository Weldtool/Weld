/// Integration tests for issue #29: task ↔ GitHub issue/PR linking and two-way sync.
///
/// Sync is inbound-only: GitHub PR/issue webhooks move the linked task between
/// the project's role columns (merged → Done, closed unmerged → In Progress,
/// reopened → Review). Weld never posts comments back to GitHub. All inbound
/// paths (webhook → task auto-move, link/unlink endpoints) use the real database.
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

// ── shared helpers ─────────────────────────────────────────────────────────

async fn setup_app() -> Router {
    dotenvy::dotenv().ok();
    let config = weld_server::config::Config::from_env();
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("db connection");
    let state = weld_server::state::AppState {
        db: pool,
        jwt_secret: config.jwt_secret,
        secure_cookies: false,
        hub: std::sync::Arc::new(weld_server::realtime::Hub::new()),
        github: None,
        master_encryption_key: config.master_encryption_key,
        frontend_url: config.frontend_url,
        study_mode: false,
        agent: None,
        study_export_token: None,
        study_require_access_code: false,
    };
    weld_server::routes::router().with_state(state)
}

const TEST_WEBHOOK_SECRET: &str = "sync_test_webhook_secret";

async fn setup_app_with_github() -> Router {
    dotenvy::dotenv().ok();
    let config = weld_server::config::Config::from_env();
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("db connection");
    let gh_cfg = weld_server::state::GithubConfig {
        app_id: 1,
        app_private_key_pem: vec![],
        webhook_secret: TEST_WEBHOOK_SECRET.to_string(),
        oauth_client_id: String::new(),
        oauth_client_secret: String::new(),
        app_slug: String::new(),
    };
    let state = weld_server::state::AppState {
        db: pool,
        jwt_secret: config.jwt_secret,
        secure_cookies: false,
        hub: std::sync::Arc::new(weld_server::realtime::Hub::new()),
        github: Some(std::sync::Arc::new(gh_cfg)),
        master_encryption_key: config.master_encryption_key,
        frontend_url: config.frontend_url,
        study_mode: false,
        agent: None,
        study_export_token: None,
        study_require_access_code: false,
    };
    weld_server::routes::router().with_state(state)
}

fn webhook_signature(secret: &str, body: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

async fn register_and_login(app: &Router) -> (String, Uuid) {
    let email = format!("sync-test-{}@example.com", Uuid::new_v4());
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"email": email, "password": "secure_password_123", "display_name": "Test"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"email": email, "password": "secure_password_123"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = v["access_token"].as_str().unwrap().to_string();

    let me_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/users/me")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let me_body = axum::body::to_bytes(me_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let me: serde_json::Value = serde_json::from_slice(&me_body).unwrap();
    let user_id = Uuid::parse_str(me["id"].as_str().unwrap()).unwrap();
    (token, user_id)
}

async fn create_team(app: &Router, token: &str) -> Uuid {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teams")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"name": format!("Team-{}", Uuid::new_v4())}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    Uuid::parse_str(v["id"].as_str().unwrap()).unwrap()
}

async fn create_project(app: &Router, token: &str, team_id: Uuid) -> serde_json::Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{team_id}/projects"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"name": format!("sync-proj-{}", Uuid::new_v4())}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn get_board(app: &Router, token: &str, project_id: Uuid) -> serde_json::Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/projects/{project_id}/board"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_task(
    app: &Router,
    token: &str,
    project_id: Uuid,
    column_id: Uuid,
) -> serde_json::Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/projects/{project_id}/tasks"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"title": "Test task", "column_id": column_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

// ── link / unlink ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_link_task_to_github_issue() {
    let app = setup_app().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    let board = get_board(&app, &token, project_id).await;
    let col_id = Uuid::parse_str(board["columns"][0]["id"].as_str().unwrap()).unwrap();
    let task = create_task(&app, &token, project_id, col_id).await;
    let task_id = task["id"].as_str().unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{task_id}/link"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"repo": "org/repo", "issue_number": 42}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["github_repo"], "org/repo");
    assert_eq!(v["github_issue_id"], 42);
    assert!(
        v["short_id"].as_i64().is_some(),
        "short_id should be present"
    );
}

#[tokio::test]
async fn test_link_requires_issue_or_pr_number() {
    let app = setup_app().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    let board = get_board(&app, &token, project_id).await;
    let col_id = Uuid::parse_str(board["columns"][0]["id"].as_str().unwrap()).unwrap();
    let task = create_task(&app, &token, project_id, col_id).await;
    let task_id = task["id"].as_str().unwrap();

    // Neither issue_number nor pr_number provided
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{task_id}/link"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"repo": "org/repo"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_unlink_task() {
    let app = setup_app().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    let board = get_board(&app, &token, project_id).await;
    let col_id = Uuid::parse_str(board["columns"][0]["id"].as_str().unwrap()).unwrap();
    let task = create_task(&app, &token, project_id, col_id).await;
    let task_id = task["id"].as_str().unwrap();

    // Link first
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{task_id}/link"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"repo": "org/repo", "pr_number": 7}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Then unlink
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/tasks/{task_id}/link"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v["github_repo"].is_null());
    assert!(v["github_issue_id"].is_null());
    assert!(v["github_pr_id"].is_null());
}

// ── inbound webhook → auto-close ──────────────────────────────────────────

#[tokio::test]
async fn test_issue_closed_auto_moves_task_to_done() {
    let app = setup_app_with_github().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    let board = get_board(&app, &token, project_id).await;
    let backlog_id = Uuid::parse_str(board["columns"][0]["id"].as_str().unwrap()).unwrap();
    let done_id = Uuid::parse_str(
        board["columns"].as_array().unwrap().last().unwrap()["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    // Create task in backlog and link it to issue #1
    let task = create_task(&app, &token, project_id, backlog_id).await;
    let task_id = task["id"].as_str().unwrap();
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{task_id}/link"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"repo": "org/repo", "issue_number": 1001}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Configure project: enable auto-close and set done_column_id
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{project_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "auto_close_on_issue_closed": true,
                        "done_column_id": done_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Send a webhook: issue #1001 closed
    let body = json!({
        "action": "closed",
        "issue": { "number": 1001 },
        "repository": { "full_name": "org/repo" }
    })
    .to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body.as_bytes());

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "issues")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                .header("X-Hub-Signature-256", sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    // Verify task is now in the Done column
    let task_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{task_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let task_body = axum::body::to_bytes(task_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&task_body).unwrap();
    assert_eq!(
        updated["column_id"].as_str().unwrap(),
        done_id.to_string(),
        "Task should have been moved to Done column"
    );
}

#[tokio::test]
async fn test_pr_merged_auto_moves_task_to_done() {
    let app = setup_app_with_github().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    let board = get_board(&app, &token, project_id).await;
    let backlog_id = Uuid::parse_str(board["columns"][0]["id"].as_str().unwrap()).unwrap();
    let done_id = Uuid::parse_str(
        board["columns"].as_array().unwrap().last().unwrap()["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let task = create_task(&app, &token, project_id, backlog_id).await;
    let task_id = task["id"].as_str().unwrap();

    // Link task to PR #2002
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{task_id}/link"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"repo": "org/repo", "pr_number": 2002}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Enable auto-close
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{project_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "auto_close_on_issue_closed": true,
                        "done_column_id": done_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Send pull_request.closed + merged=true webhook
    let body = json!({
        "action": "closed",
        "pull_request": { "number": 2002, "merged": true },
        "repository": { "full_name": "org/repo" }
    })
    .to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body.as_bytes());

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "pull_request")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                .header("X-Hub-Signature-256", sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let task_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{task_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let task_body = axum::body::to_bytes(task_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&task_body).unwrap();
    assert_eq!(updated["column_id"].as_str().unwrap(), done_id.to_string());
}

// ── project settings ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_project_update_auto_close_settings() {
    let app = setup_app().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    let board = get_board(&app, &token, project_id).await;
    let done_col_id = board["columns"].as_array().unwrap().last().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{project_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "auto_close_on_issue_closed": true,
                        "done_column_id": done_col_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["auto_close_on_issue_closed"], true);
    assert_eq!(v["done_column_id"].as_str().unwrap(), done_col_id);
}

// ── push webhook auto-link ─────────────────────────────────────────────────

#[tokio::test]
async fn test_push_webhook_autolinks_task_by_short_id() {
    let app = setup_app_with_github().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    let board = get_board(&app, &token, project_id).await;
    let col_id = Uuid::parse_str(board["columns"][0]["id"].as_str().unwrap()).unwrap();

    let task = create_task(&app, &token, project_id, col_id).await;
    let task_id = task["id"].as_str().unwrap();
    let short_id = task["short_id"].as_i64().unwrap();

    let push_body = json!({
        "commits": [{
            "id": "abc123",
            "message": format!("fix: resolve issue [WD-{short_id}]"),
            "added": [], "removed": [], "modified": []
        }],
        "repository": { "full_name": "org/auto-link-repo" }
    })
    .to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, push_body.as_bytes());

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "push")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                .header("X-Hub-Signature-256", sig)
                .body(Body::from(push_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    // Verify github_repo was set
    let task_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{task_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let task_body = axum::body::to_bytes(task_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&task_body).unwrap();
    assert_eq!(updated["github_repo"], "org/auto-link-repo");
}
