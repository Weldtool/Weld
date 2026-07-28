use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_app() -> Router {
    dotenvy::dotenv().ok();
    let config = weld_server::config::Config::from_env();
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to the database");
    let state = weld_server::state::AppState {
        db: pool,
        jwt_secret: config.jwt_secret,
        secure_cookies: false,
        hub: std::sync::Arc::new(weld_server::realtime::Hub::new()),
        github: None, // GitHub API calls are not exercised in unit tests
        master_encryption_key: config.master_encryption_key,
        frontend_url: config.frontend_url,
        study_mode: false,
        agent: None,
        study_export_token: None,
        study_require_access_code: false,
    };
    weld_server::routes::router().with_state(state)
}

const TEST_WEBHOOK_SECRET: &str = "test_webhook_secret_do_not_use_in_prod";

async fn setup_app_with_github() -> Router {
    dotenvy::dotenv().ok();
    let config = weld_server::config::Config::from_env();
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to the database");
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

/// Computes the `X-Hub-Signature-256` header value for a given body.
fn webhook_signature(secret: &str, body: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Registers a user, logs in, and returns (access_token, user_id).
async fn register_and_login(app: &Router) -> (String, Uuid) {
    let email = format!("github-test-{}@example.com", Uuid::new_v4());
    let password = "secure_password_123";

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "password": password,
                        "display_name": "GitHub Test User"
                    })
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
                    json!({"email": email, "password": password}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = login_json["access_token"].as_str().unwrap().to_string();

    // Fetch user id
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
    let me_bytes = axum::body::to_bytes(me_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let me_json: serde_json::Value = serde_json::from_slice(&me_bytes).unwrap();
    let user_id = Uuid::parse_str(me_json["id"].as_str().unwrap()).unwrap();

    (token, user_id)
}

/// Creates a team owned by the caller and returns the team_id.
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
                    json!({"name": format!("GitHub Test Team {}", Uuid::new_v4())}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let j: serde_json::Value = serde_json::from_slice(&body).unwrap();
    Uuid::parse_str(j["id"].as_str().unwrap()).unwrap()
}

// ── install-url ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_install_url_requires_auth() {
    let app = setup_app().await;
    let team_id = Uuid::new_v4();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{team_id}/github/install-url"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_install_url_requires_admin() {
    let app = setup_app().await;
    let (owner_token, _) = register_and_login(&app).await;
    let (member_token, member_id) = register_and_login(&app).await;
    let team_id = create_team(&app, &owner_token).await;

    // Add second user as Member (not Admin)
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{team_id}/members"))
                .header("Authorization", format!("Bearer {owner_token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"user_id": member_id, "role": "member"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{team_id}/github/install-url"))
                .header("Authorization", format!("Bearer {member_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_install_url_returns_400_when_github_not_configured() {
    let app = setup_app().await; // github: None
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{team_id}/github/install-url"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // A server without a GitHub App is a deployment state, not a crash: the UI
    // needs to tell the user "GitHub isn't set up", which a 500 cannot express.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── setup callback ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_setup_callback_rejects_invalid_state() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/github/setup?installation_id=12345&state=not-a-valid-jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Bad state token → 400
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_setup_callback_rejects_expired_state() {
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;

    dotenvy::dotenv().ok();
    let secret = std::env::var("JWT_SECRET").unwrap();

    // Manually craft an expired install state token
    let expired_claims = json!({
        "sub": Uuid::new_v4().to_string(),
        "kind": "install",
        "exp": chrono::Utc::now().timestamp() - 3600,
    });
    let state_token = encode(
        &Header::default(),
        &expired_claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .unwrap();

    let app = setup_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/github/setup?installation_id=12345&state={state_token}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── oauth/url ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_oauth_url_requires_auth() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/github/oauth/url")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_oauth_url_returns_400_when_github_not_configured() {
    let app = setup_app().await; // github: None
    let (token, _) = register_and_login(&app).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/github/oauth/url")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── oauth/callback ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_oauth_callback_rejects_invalid_state() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/github/oauth/callback?code=abc123&state=garbage")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── GET /teams/:id/github ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_connection_requires_auth() {
    let app = setup_app().await;
    let team_id = Uuid::new_v4();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{team_id}/github"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_connection_returns_404_when_not_connected() {
    let app = setup_app().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{team_id}/github"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── DELETE /teams/:id/github ──────────────────────────────────────────────────

#[tokio::test]
async fn test_disconnect_requires_admin() {
    let app = setup_app().await;
    let (owner_token, _) = register_and_login(&app).await;
    let (viewer_token, viewer_id) = register_and_login(&app).await;
    let team_id = create_team(&app, &owner_token).await;

    // Add viewer
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{team_id}/members"))
                .header("Authorization", format!("Bearer {owner_token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"user_id": viewer_id, "role": "viewer"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/teams/{team_id}/github"))
                .header("Authorization", format!("Bearer {viewer_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_disconnect_returns_404_when_not_connected() {
    let app = setup_app().await;
    let (token, _) = register_and_login(&app).await;
    let team_id = create_team(&app, &token).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/teams/{team_id}/github"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── webhook ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_webhook_github_not_configured_returns_400() {
    let app = setup_app().await; // github: None
    let body = json!({"action": "opened"}).to_string();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "issues")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                .header("X-Hub-Signature-256", "sha256=abc123")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    // No App configured means no webhook secret to verify against — retrying
    // will not help, so this is a 4xx rather than a 5xx GitHub would redeliver.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_webhook_missing_signature_returns_401() {
    let app = setup_app_with_github().await;
    let body = json!({"action": "opened"}).to_string();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "issues")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                // intentionally omit X-Hub-Signature-256
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_webhook_bad_signature_returns_401() {
    let app = setup_app_with_github().await;
    let body = json!({"action": "opened"}).to_string();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "issues")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                .header(
                    "X-Hub-Signature-256",
                    "sha256=deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_webhook_valid_ping_event() {
    let app = setup_app_with_github().await;
    let body = json!({"zen": "Non-blocking is better than blocking."}).to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body.as_bytes());

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "ping")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                .header("X-Hub-Signature-256", sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_replay_protection() {
    let app = setup_app_with_github().await;
    let body = json!({"action": "opened"}).to_string();
    let delivery_id = Uuid::new_v4().to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body.as_bytes());

    // First delivery — processed
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "ping")
                .header("X-GitHub-Delivery", &delivery_id)
                .header("X-Hub-Signature-256", &sig)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Second delivery — same id, returns 200 without reprocessing
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "ping")
                .header("X-GitHub-Delivery", &delivery_id)
                .header("X-Hub-Signature-256", &sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_issues_opened_event() {
    let app = setup_app_with_github().await;
    let body = json!({
        "action": "opened",
        "issue": { "number": 42, "title": "Bug report" },
        "repository": { "full_name": "org/repo" }
    })
    .to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body.as_bytes());

    let resp = app
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
}

#[tokio::test]
async fn test_webhook_pull_request_opened_event() {
    let app = setup_app_with_github().await;
    let body = json!({
        "action": "opened",
        "pull_request": { "number": 7, "title": "feat: new thing" },
        "repository": { "full_name": "org/repo" }
    })
    .to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body.as_bytes());

    let resp = app
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
}

#[tokio::test]
async fn test_webhook_push_with_task_ref_event() {
    let app = setup_app_with_github().await;
    let body = json!({
        "commits": [
            {
                "id": "abc123",
                "message": "fix: resolve issue [WD-42]",
                "added": [], "removed": [], "modified": []
            }
        ],
        "repository": { "full_name": "org/repo" }
    })
    .to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body.as_bytes());

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "push")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                .header("X-Hub-Signature-256", sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_installation_deleted_event() {
    let app = setup_app_with_github().await;
    let body = json!({
        "action": "deleted",
        "installation": { "id": 999999999_i64 },
        "repository": { "full_name": "org/repo" }
    })
    .to_string();
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body.as_bytes());

    // No matching github_connections row — handler should log and return 200
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "installation")
                .header("X-GitHub-Delivery", Uuid::new_v4().to_string())
                .header("X-Hub-Signature-256", sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_failure_recorded_in_deliveries() {
    let app = setup_app_with_github().await;
    // Malformed JSON — body is valid bytes but not valid JSON, so handler returns 400
    let body = b"not valid json at all";
    let sig = webhook_signature(TEST_WEBHOOK_SECRET, body);
    let delivery_id = format!("fail-test-{}", Uuid::new_v4());

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/github/webhook")
                .header("Content-Type", "application/json")
                .header("X-GitHub-Event", "issues")
                .header("X-GitHub-Delivery", &delivery_id)
                .header("X-Hub-Signature-256", sig)
                .body(Body::from(body.as_slice()))
                .unwrap(),
        )
        .await
        .unwrap();
    // Bad JSON → Validation error → 400
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── state token helpers ───────────────────────────────────────────────────────

#[test]
fn test_state_token_round_trip() {
    use weld_server::services::github::helpers::{sign_state_token, verify_state_token};

    let secret = "test-jwt-secret";
    let team_id = Uuid::new_v4();

    let token = sign_state_token(secret, team_id, "install").unwrap();
    let decoded = verify_state_token(secret, &token, "install").unwrap();
    assert_eq!(decoded, team_id);
}

#[test]
fn test_state_token_kind_mismatch_is_rejected() {
    use weld_server::services::github::helpers::{sign_state_token, verify_state_token};

    let secret = "test-jwt-secret";
    let id = Uuid::new_v4();

    let token = sign_state_token(secret, id, "install").unwrap();
    let result = verify_state_token(secret, &token, "oauth");
    assert!(result.is_err());
}

#[test]
fn test_state_token_wrong_secret_is_rejected() {
    use weld_server::services::github::helpers::{sign_state_token, verify_state_token};

    let id = Uuid::new_v4();
    let token = sign_state_token("secret-a", id, "install").unwrap();
    let result = verify_state_token("secret-b", &token, "install");
    assert!(result.is_err());
}
