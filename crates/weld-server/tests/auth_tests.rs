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
    };
    weld_server::routes::router().with_state(state)
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
