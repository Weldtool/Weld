/// Integration tests for issue #29: task ↔ GitHub issue/PR linking and two-way sync.
///
/// Inbound: GitHub PR/issue webhooks move the linked task between the project's
/// role columns (merged → Done, closed unmerged → In Progress, reopened →
/// Review). Every delivery is scoped to the teams that own the sending
/// installation — see `test_webhook_does_not_cross_tenants`. All inbound paths
/// (webhook → task auto-move, link/unlink endpoints) use the real database.
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

/// Records a GitHub App installation for `team_id`.
///
/// Webhook deliveries are matched to teams through this table, so a team with
/// no connection receives nothing — which is the point of the scoping.
async fn connect_installation(team_id: Uuid, installation_id: i64, owner: Uuid) {
    dotenvy::dotenv().ok();
    let config = weld_server::config::Config::from_env();
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("db connection");
    sqlx::query(
        "INSERT INTO github_connections (team_id, installation_id, account_login, created_by)
         VALUES ($1, $2, 'test-org', $3)
         ON CONFLICT (team_id) DO UPDATE SET installation_id = EXCLUDED.installation_id",
    )
    .bind(team_id)
    .bind(installation_id)
    .bind(owner)
    .execute(&pool)
    .await
    .expect("insert github connection");
}

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
    let (token, user_id) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    connect_installation(team_id, 900001, user_id).await;
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
        "repository": { "full_name": "org/repo" },
        "installation": { "id": 900001 }
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
    let (token, user_id) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    connect_installation(team_id, 900002, user_id).await;
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
        "repository": { "full_name": "org/repo" },
        "installation": { "id": 900002 }
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
    let (token, user_id) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    connect_installation(team_id, 900003, user_id).await;
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
        "repository": { "full_name": "org/auto-link-repo" },
        "installation": { "id": 900003 }
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

// ── tenant scoping ─────────────────────────────────────────────────────────

/// A webhook must only touch boards belonging to the installation that sent it.
///
/// `github_repo` is free text, so two unrelated teams can both have tasks
/// pointing at "org/shared-name". Before deliveries were scoped by installation,
/// one team's merge moved the *other* team's card — a cross-tenant write
/// reachable by anyone willing to name their repo the same thing.
#[tokio::test]
async fn test_webhook_does_not_cross_tenants() {
    let app = setup_app_with_github().await;
    const SHARED_REPO: &str = "org/shared-name";

    // Team A installs the App; team B does not (or installs a different one).
    let (token_a, user_a) = register_and_login(&app).await;
    let team_a = create_team(&app, &token_a).await;
    connect_installation(team_a, 910001, user_a).await;

    let (token_b, user_b) = register_and_login(&app).await;
    let team_b = create_team(&app, &token_b).await;
    connect_installation(team_b, 910002, user_b).await;

    // Both teams have a task linked to a PR #4242 in a repo of the same name.
    let mut cards = Vec::new();
    for (token, team_id) in [(&token_a, team_a), (&token_b, team_b)] {
        let project = create_project(&app, token, team_id).await;
        let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
        let board = get_board(&app, token, project_id).await;
        let backlog_id = Uuid::parse_str(board["columns"][0]["id"].as_str().unwrap()).unwrap();
        let done_id = Uuid::parse_str(
            board["columns"].as_array().unwrap().last().unwrap()["id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

        let task = create_task(&app, token, project_id, backlog_id).await;
        let task_id = task["id"].as_str().unwrap().to_string();

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/tasks/{task_id}/link"))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        json!({"repo": SHARED_REPO, "pr_number": 4242}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        app.clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/projects/{project_id}"))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        json!({"auto_close_on_issue_closed": true, "done_column_id": done_id})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        cards.push((token.clone(), task_id, backlog_id, done_id));
    }

    // Team A's installation reports the PR as merged.
    let body = json!({
        "action": "closed",
        "pull_request": { "number": 4242, "merged": true },
        "repository": { "full_name": SHARED_REPO },
        "installation": { "id": 910001 }
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

    let column_of = async |token: &str, task_id: &str| -> String {
        let resp = app
            .clone()
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
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let task: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        task["column_id"].as_str().unwrap().to_string()
    };

    let (token, task_id, _, done_id) = &cards[0];
    assert_eq!(
        column_of(token, task_id).await,
        done_id.to_string(),
        "team A's card should advance — the delivery came from its own installation"
    );

    let (token, task_id, backlog_id, _) = &cards[1];
    assert_eq!(
        column_of(token, task_id).await,
        backlog_id.to_string(),
        "team B's card must NOT move: the webhook came from another tenant's installation"
    );
}

/// A `check_suite` webhook records CI state on the card linked to its PR.
#[tokio::test]
async fn test_check_suite_sets_task_check_status() {
    let app = setup_app_with_github().await;
    let (token, user_id) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    connect_installation(team_id, 920001, user_id).await;

    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    let board = get_board(&app, &token, project_id).await;
    let col_id = Uuid::parse_str(board["columns"][0]["id"].as_str().unwrap()).unwrap();

    let task = create_task(&app, &token, project_id, col_id).await;
    let task_id = task["id"].as_str().unwrap();
    assert!(
        task["github_check_status"].is_null(),
        "a new task has no CI result"
    );

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{task_id}/link"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"repo": "org/ci-repo", "pr_number": 7007}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json!({
        "action": "completed",
        "check_suite": {
            "conclusion": "failure",
            "pull_requests": [{ "number": 7007 }]
        },
        "repository": { "full_name": "org/ci-repo" },
        "installation": { "id": 920001 }
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
                .header("X-GitHub-Event", "check_suite")
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
    let bytes = axum::body::to_bytes(task_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["github_check_status"], "failure");
}

// ── outbound comment toggle ────────────────────────────────────────────────

/// `github_comment_on_move` defaults on and round-trips through PATCH.
///
/// The comment itself needs a live GitHub, but the setting that gates it is
/// ours — and a toggle that silently fails to persist is worse than no toggle.
#[tokio::test]
async fn test_github_comment_on_move_defaults_on_and_toggles() {
    let app = setup_app().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;
    let project = create_project(&app, &token, team_id).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    assert_eq!(
        project["github_comment_on_move"], true,
        "commenting should be on by default"
    );

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{project_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "github_comment_on_move": false }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["github_comment_on_move"], false);

    // Unrelated PATCHes must not silently switch it back on.
    let resp = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{project_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({ "name": "Renamed" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let after: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(after["github_comment_on_move"], false);
}
