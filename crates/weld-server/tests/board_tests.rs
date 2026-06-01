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
        study_require_access_code: false,
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

async fn add_member(app: &Router, caller: &TestUser, team_id: Uuid, new_user_id: Uuid, role: &str) {
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
    assert!(
        resp.status().is_success(),
        "expected member added, got {}",
        resp.status()
    );
}

async fn create_project(app: &Router, user: &TestUser, team_id: Uuid, name: &str) -> Uuid {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/projects", team_id))
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

async fn create_column(app: &Router, user: &TestUser, project_id: Uuid, name: &str) -> Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/projects/{}/columns", project_id))
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
    serde_json::from_slice(&body).unwrap()
}

async fn create_task(
    app: &Router,
    user: &TestUser,
    project_id: Uuid,
    column_id: Uuid,
    title: &str,
) -> Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/projects/{}/tasks", project_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", user.access_token))
                .body(Body::from(
                    json!({"title": title, "column_id": column_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "create_task failed: status={}",
        resp.status()
    );
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn create_column_succeeds_for_admin_and_appends_position() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "BoardCoAdmin").await;
    let project_id = create_project(&app, &owner, team_id, "BoardP").await;

    let first = create_column(&app, &owner, project_id, "Backlog").await;
    let second = create_column(&app, &owner, project_id, "Doing").await;

    let p1 = first["position"].as_f64().unwrap();
    let p2 = second["position"].as_f64().unwrap();
    assert!(p2 > p1, "expected second column position to be greater");
}

#[tokio::test]
async fn create_column_forbidden_for_member() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let member = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "BoardCoMem").await;
    add_member(&app, &owner, team_id, member.id, "member").await;
    let project_id = create_project(&app, &owner, team_id, "BoardP2").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/projects/{}/columns", project_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", member.access_token))
                .body(Body::from(json!({"name": "Sneaky"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn delete_column_relocates_active_tasks_to_target() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "BoardCoRelocate").await;
    let project_id = create_project(&app, &owner, team_id, "BoardP3").await;

    let source = create_column(&app, &owner, project_id, "Doing").await;
    let source_id = Uuid::parse_str(source["id"].as_str().unwrap()).unwrap();
    let target = create_column(&app, &owner, project_id, "Parking").await;
    let target_id = Uuid::parse_str(target["id"].as_str().unwrap()).unwrap();

    let task = create_task(&app, &owner, project_id, source_id, "T1").await;
    let task_id = task["id"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/columns/{}?move_to={}",
                    source_id, target_id
                ))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // The task should now live in the target column.
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{}", task_id))
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
    let moved: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(moved["column_id"].as_str().unwrap(), target_id.to_string());
}

#[tokio::test]
async fn delete_column_defaults_relocation_to_backlog() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "BoardCoBacklog").await;
    // New projects auto-seed a "Backlog" column, which is the default target.
    let project_id = create_project(&app, &owner, team_id, "BoardP4").await;

    let source = create_column(&app, &owner, project_id, "Doing").await;
    let source_id = Uuid::parse_str(source["id"].as_str().unwrap()).unwrap();
    let task = create_task(&app, &owner, project_id, source_id, "T1").await;
    let task_id = task["id"].as_str().unwrap().to_string();

    // Delete with no move_to — tasks fall back to the Backlog column.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/columns/{}", source_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Resolve the Backlog column id from the board to verify the relocation.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/projects/{}/board", project_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let board: Value = serde_json::from_slice(&body).unwrap();
    let backlog_id = board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"].as_str() == Some("Backlog"))
        .and_then(|c| c["id"].as_str())
        .unwrap()
        .to_string();

    let task = board["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"].as_str() == Some(task_id.as_str()))
        .unwrap();
    assert_eq!(task["column_id"].as_str().unwrap(), backlog_id);
}

#[tokio::test]
async fn create_label_succeeds_and_unique_per_project() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "LabelCo").await;
    let project_id = create_project(&app, &owner, team_id, "LabelP").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/projects/{}/labels", project_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(
                    json!({"name": "bug", "color": "#ff0000"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/projects/{}/labels", project_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(
                    json!({"name": "bug", "color": "#ff0000"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_label_forbidden_for_viewer() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let viewer = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "ViewerLab").await;
    add_member(&app, &owner, team_id, viewer.id, "viewer").await;
    let project_id = create_project(&app, &owner, team_id, "VLP").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/projects/{}/labels", project_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", viewer.access_token))
                .body(Body::from(
                    json!({"name": "wontfix", "color": "#000000"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_task_forbidden_for_viewer() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let viewer = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "ViewerTask").await;
    add_member(&app, &owner, team_id, viewer.id, "viewer").await;
    let project_id = create_project(&app, &owner, team_id, "VTP").await;
    let column = create_column(&app, &owner, project_id, "Backlog").await;
    let column_id = Uuid::parse_str(column["id"].as_str().unwrap()).unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/projects/{}/tasks", project_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", viewer.access_token))
                .body(Body::from(
                    json!({"title": "Sneaky", "column_id": column_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn task_lifecycle_create_get_update_soft_delete() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "TaskLifeco").await;
    let project_id = create_project(&app, &owner, team_id, "TLP").await;
    let column = create_column(&app, &owner, project_id, "Backlog").await;
    let column_id = Uuid::parse_str(column["id"].as_str().unwrap()).unwrap();

    let task = create_task(&app, &owner, project_id, column_id, "First").await;
    let task_id = Uuid::parse_str(task["id"].as_str().unwrap()).unwrap();
    assert_eq!(task["priority"], "none");
    assert_eq!(task["reporter_id"], owner.id.to_string());

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(
                    json!({"title": "Renamed", "priority": "high"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["title"], "Renamed");
    assert_eq!(updated["priority"], "high");

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn move_task_places_between_neighbours() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "MoveCo").await;
    let project_id = create_project(&app, &owner, team_id, "MP").await;
    let backlog = create_column(&app, &owner, project_id, "Backlog").await;
    let doing = create_column(&app, &owner, project_id, "Doing").await;
    let backlog_id = Uuid::parse_str(backlog["id"].as_str().unwrap()).unwrap();
    let doing_id = Uuid::parse_str(doing["id"].as_str().unwrap()).unwrap();

    // Two anchor tasks in Doing at positions 1.0 and 2.0.
    let a = create_task(&app, &owner, project_id, doing_id, "A").await;
    let b = create_task(&app, &owner, project_id, doing_id, "B").await;
    let pos_a = a["position"].as_f64().unwrap();
    let pos_b = b["position"].as_f64().unwrap();
    assert!(pos_b > pos_a);

    // Task to move starts in Backlog.
    let mover = create_task(&app, &owner, project_id, backlog_id, "Mover").await;
    let mover_id = Uuid::parse_str(mover["id"].as_str().unwrap()).unwrap();

    let target_pos = (pos_a + pos_b) / 2.0;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{}/move", mover_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(
                    json!({"column_id": doing_id, "position": target_pos}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let moved: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(moved["column_id"], doing_id.to_string());
    let new_pos = moved["position"].as_f64().unwrap();
    assert!(new_pos > pos_a && new_pos < pos_b);
}

#[tokio::test]
async fn move_task_rejects_cross_project_column() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "XPCo").await;
    let p1 = create_project(&app, &owner, team_id, "P1").await;
    let p2 = create_project(&app, &owner, team_id, "P2").await;
    let c1 = create_column(&app, &owner, p1, "Backlog").await;
    let c2 = create_column(&app, &owner, p2, "Backlog").await;
    let c1_id = Uuid::parse_str(c1["id"].as_str().unwrap()).unwrap();
    let c2_id = Uuid::parse_str(c2["id"].as_str().unwrap()).unwrap();

    let task = create_task(&app, &owner, p1, c1_id, "T").await;
    let task_id = Uuid::parse_str(task["id"].as_str().unwrap()).unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{}/move", task_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(
                    json!({"column_id": c2_id, "position": 1.0}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_board_excludes_soft_deleted_tasks() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "BoardCo").await;
    let project_id = create_project(&app, &owner, team_id, "BBP").await;
    let column = create_column(&app, &owner, project_id, "Backlog").await;
    let column_id = Uuid::parse_str(column["id"].as_str().unwrap()).unwrap();

    let live = create_task(&app, &owner, project_id, column_id, "Live").await;
    let dead = create_task(&app, &owner, project_id, column_id, "Dead").await;
    let dead_id = Uuid::parse_str(dead["id"].as_str().unwrap()).unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/tasks/{}", dead_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/projects/{}/board", project_id))
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
    let board: Value = serde_json::from_slice(&body).unwrap();
    let ids: Vec<&str> = board["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&live["id"].as_str().unwrap()));
    assert!(!ids.contains(&dead["id"].as_str().unwrap()));
}

#[tokio::test]
async fn get_task_forbidden_for_non_member() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let outsider = register_and_login(&app).await;
    let team_id = create_team(&app, &owner, "PrivCo").await;
    let project_id = create_project(&app, &owner, team_id, "PP").await;
    let column = create_column(&app, &owner, project_id, "Backlog").await;
    let column_id = Uuid::parse_str(column["id"].as_str().unwrap()).unwrap();
    let task = create_task(&app, &owner, project_id, column_id, "Secret").await;
    let task_id = Uuid::parse_str(task["id"].as_str().unwrap()).unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("Authorization", format!("Bearer {}", outsider.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
