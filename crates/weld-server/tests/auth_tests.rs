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
        github: None,
        master_encryption_key: config.master_encryption_key,
        frontend_url: config.frontend_url,
        study_mode: false,
        agent: None,
        study_export_token: None,
    };
    weld_server::routes::router().with_state(state)
}

/// Helper: registers a user and logs in, returning (access_token, refresh_token_cookie)
async fn register_and_login(app: &Router) -> (String, String) {
    let email = format!("test-{}@example.com", Uuid::new_v4());
    let password = "secure_password_123";

    // Register
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
                        "display_name": "Test User"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Login
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "password": password
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let access_token = json["access_token"].as_str().unwrap().to_string();

    // Extract just the "refresh_token=<value>" part for use in Cookie header
    let refresh_cookie = cookie.split(';').next().unwrap().to_string();

    (access_token, refresh_cookie)
}

#[tokio::test]
async fn test_login_success() {
    let app = setup_app().await;
    let email = format!("test-{}@example.com", Uuid::new_v4());
    let password = "secure_password_123";

    // First, register a new user
    let register_payload = json!({
        "email": email,
        "password": password,
        "display_name": "Test User"
    });
    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(register_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register_response.status(), StatusCode::CREATED);

    // Now, attempt to login with the correct credentials
    let login_payload = json!({
            "email": email,
            "password": "secure_password_123"
    });
    let login_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(login_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_response.status(), StatusCode::OK);
    assert!(login_response.headers().get("set-cookie").is_some());
    let body = axum::body::to_bytes(login_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("access_token").is_some());
    assert_eq!(json["token_type"], "Bearer");
}

#[tokio::test]
async fn test_login_wrong_password() {
    let app = setup_app().await;
    let email = format!("test-{}@example.com", Uuid::new_v4());
    let password = "secure_password_123";

    // First, register a new User
    let register_payload = json!({
        "email": email,
        "password": password,
        "display_name": "Test User"
    });
    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(register_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register_response.status(), StatusCode::CREATED);

    // Now, attempt to login with the wrong password
    let login_payload = json!({
        "email": email,
        "password": "wrong_password"
    });
    let login_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(login_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let app = setup_app().await;
    let email = format!("test-{}@example.com", Uuid::new_v4());
    let password = "some_password";

    // Attempt to login with a non-existent User
    let login_payload = json!({
        "email": email,
        "password": password
    });
    let login_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(login_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_response.status(), StatusCode::UNAUTHORIZED);
}

// --- Protected route tests ---

#[tokio::test]
async fn test_users_me_with_valid_token() {
    let app = setup_app().await;
    let (access_token, _) = register_and_login(&app).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/users/me")
                .header("Authorization", format!("Bearer {}", access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("id").is_some());
    assert!(json.get("email").is_some());
    assert!(json.get("display_name").is_some());
}

#[tokio::test]
async fn test_users_me_without_token() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/users/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_users_me_with_garbage_token() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/users/me")
                .header("Authorization", "Bearer not-a-real-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_users_me_with_expired_token() {
    let app = setup_app().await;

    // Craft a JWT that expired in the past
    dotenvy::dotenv().ok();
    let secret = std::env::var("JWT_SECRET").unwrap();
    let claims = weld_server::middleware::auth::Claims {
        sub: Uuid::new_v4().to_string(),
        role: "member".to_string(),
        iat: chrono::Utc::now().timestamp() - 3600,
        exp: chrono::Utc::now().timestamp() - 1800,
    };
    let expired_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_ref()),
    )
    .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/users/me")
                .header("Authorization", format!("Bearer {}", expired_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// --- Refresh token tests ---

#[tokio::test]
async fn test_refresh_token_success() {
    let app = setup_app().await;
    let (_, refresh_cookie) = register_and_login(&app).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("Cookie", &refresh_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    assert!(resp.headers().get("set-cookie").is_some());
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("access_token").is_some());
    assert_eq!(json["token_type"], "Bearer");
}

#[tokio::test]
async fn test_refresh_token_rotation_prevents_reuse() {
    let app = setup_app().await;
    let (_, refresh_cookie) = register_and_login(&app).await;

    // First refresh — should succeed
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("Cookie", &refresh_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Second refresh with the same cookie — should fail (token was rotated)
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("Cookie", &refresh_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_refresh_without_cookie() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// --- Logout tests ---

#[tokio::test]
async fn test_logout_invalidates_refresh_token() {
    let app = setup_app().await;
    let (_, refresh_cookie) = register_and_login(&app).await;

    // Logout
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .header("Cookie", &refresh_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Try to refresh with the same cookie — should fail
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("Cookie", &refresh_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
