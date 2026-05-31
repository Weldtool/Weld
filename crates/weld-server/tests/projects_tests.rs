use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
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

struct TestUser {
    id: Uuid,
    access_token: String,
}

async fn register_and_login(app: &Router) -> TestUser {
    let email = format!("test-{}@example.com", Uuid::new_v4());
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
                        "display_name": "Test User"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let id = Uuid::parse_str(json["id"].as_str().unwrap()).unwrap();

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
    let json: Value = serde_json::from_slice(&body).unwrap();
    let access_token = json["access_token"].as_str().unwrap().to_string();

    TestUser { id, access_token }
}

async fn create_team(app: &Router, user: &TestUser, name: &str) -> Uuid {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teams")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", user.access_token))
                .body(Body::from(json!({ "name": name }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    Uuid::parse_str(json["id"].as_str().unwrap()).unwrap()
}

async fn add_member(
    app: &Router,
    caller: &TestUser,
    team_id: Uuid,
    new_user_id: Uuid,
    role: &str,
) -> StatusCode {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/members", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", caller.access_token))
                .body(Body::from(
                    json!({"user_id": new_user_id, "role": role}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    resp.status()
}

#[tokio::test]
async fn create_project_succeeds_for_owner() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "Acme").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(
                    json!({"name": "Apollo", "description": "Moon mission"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "Apollo");
    assert!(json["slug"].as_str().unwrap().starts_with("apollo"));
    assert_eq!(json["team_id"], team_id.to_string());
}

#[tokio::test]
async fn create_project_requires_auth() {
    let app = setup_app().await;
    let team_id = Uuid::new_v4();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"name": "X"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_project_forbidden_for_non_member() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let outsider = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "Locked").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", outsider.access_token))
                .body(Body::from(json!({"name": "Sneaky"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_project_forbidden_for_member_role() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let member = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "Members").await;

    let status = add_member(&app, &owner, team_id, member.id, "member").await;
    assert_eq!(status, StatusCode::CREATED);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", member.access_token))
                .body(Body::from(json!({"name": "NoPerms"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn list_team_projects_returns_only_team_projects() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_a = create_team(&app, &owner, "TeamA").await;
    let team_b = create_team(&app, &owner, "TeamB").await;

    for name in ["P1", "P2"] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/teams/{}/projects", team_a))
                    .header("Content-Type", "application/json")
                    .header("Authorization", format!("Bearer {}", owner.access_token))
                    .body(Body::from(json!({ "name": name }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_b))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(json!({"name": "Other"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{}/projects", team_a))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    for p in arr {
        assert_eq!(p["team_id"], team_a.to_string());
    }
}

#[tokio::test]
async fn get_project_forbidden_for_non_member() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let outsider = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "Private").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(json!({"name": "Hidden"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let project_id = serde_json::from_slice::<Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("Authorization", format!("Bearer {}", outsider.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn get_project_returns_404_for_unknown() {
    let app = setup_app().await;
    let user = register_and_login(&app).await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/projects/{}", Uuid::new_v4()))
                .header("Authorization", format!("Bearer {}", user.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_project_succeeds_for_admin() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "Editable").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(json!({"name": "Old name"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let project_id = serde_json::from_slice::<Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(
                    json!({"name": "New name", "description": "Updated"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "New name");
    assert_eq!(json["description"], "Updated");
}

#[tokio::test]
async fn update_project_forbidden_for_member() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let member = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "MemberCantEdit").await;
    add_member(&app, &owner, team_id, member.id, "member").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(json!({"name": "Project"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let project_id = serde_json::from_slice::<Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", member.access_token))
                .body(Body::from(json!({"name": "Hijacked"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn delete_project_only_owner() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let admin = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "DeleteTest").await;
    let add_status = add_member(&app, &owner, team_id, admin.id, "admin").await;
    assert_eq!(add_status, StatusCode::CREATED);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(json!({"name": "ToDelete"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create body: {}",
        String::from_utf8_lossy(&body)
    );
    let project_id = serde_json::from_slice::<Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Admin cannot delete
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("Authorization", format!("Bearer {}", admin.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Owner can delete
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // After deletion, GET returns 404
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_project_validates_name_length() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "Validator").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(json!({"name": ""}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_teams_returns_only_users_active_teams() {
    let app = setup_app().await;
    let user = register_and_login(&app).await;
    let team_a = create_team(&app, &user, "Team A").await;
    let team_b = create_team(&app, &user, "Team B").await;

    let other = register_and_login(&app).await;
    create_team(&app, &other, "Other team").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teams")
                .header("Authorization", format!("Bearer {}", user.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let teams = json.as_array().unwrap();
    // The two explicitly created teams plus the auto-created personal team.
    assert_eq!(teams.len(), 3);
    let shared: Vec<&Value> = teams
        .iter()
        .filter(|t| !t["is_personal"].as_bool().unwrap())
        .collect();
    assert_eq!(shared.len(), 2);
    let ids: Vec<Uuid> = shared
        .iter()
        .map(|t| Uuid::parse_str(t["id"].as_str().unwrap()).unwrap())
        .collect();
    assert!(ids.contains(&team_a));
    assert!(ids.contains(&team_b));
    // Personal team is sorted first and is also owned by the user.
    assert_eq!(teams[0]["is_personal"], true);
    assert_eq!(teams[0]["role"], "owner");
}

#[tokio::test]
async fn list_team_members_returns_owner_and_added_members() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "Member List Team").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{team_id}/members"))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let members = json.as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["user_id"], owner.id.to_string());
    assert_eq!(members[0]["role"], "owner");
}

#[tokio::test]
async fn list_team_members_forbidden_for_non_member() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let outsider = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "Closed Team").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{team_id}/members"))
                .header("Authorization", format!("Bearer {}", outsider.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn list_teams_requires_auth() {
    let app = setup_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teams")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
