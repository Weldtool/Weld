//! Regression tests for access-control and validation gaps found auditing the
//! API: role escalation, project-level access leaking through side doors,
//! private-channel moderation, unvalidated cross-project references, and the
//! GitHub callbacks trusting whoever arrived at them.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

async fn pool() -> sqlx::PgPool {
    dotenvy::dotenv().ok();
    let config = weld_server::config::Config::from_env();
    sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to the database")
}

async fn app_with(github: bool) -> Router {
    dotenvy::dotenv().ok();
    let config = weld_server::config::Config::from_env();
    let github = github.then(|| {
        std::sync::Arc::new(weld_server::state::GithubConfig {
            app_id: 1,
            app_private_key_pem: vec![],
            webhook_secret: "test_webhook_secret".to_string(),
            oauth_client_id: "client".to_string(),
            oauth_client_secret: "secret".to_string(),
            app_slug: "weld-test".to_string(),
        })
    });
    let state = weld_server::state::AppState {
        db: pool().await,
        jwt_secret: config.jwt_secret,
        secure_cookies: false,
        hub: std::sync::Arc::new(weld_server::realtime::Hub::new()),
        github,
        master_encryption_key: config.master_encryption_key,
        frontend_url: config.frontend_url,
        study_mode: false,
        agent: None,
        study_export_token: None,
        study_require_access_code: false,
    };
    weld_server::routes::router().with_state(state)
}

async fn setup_app() -> Router {
    app_with(false).await
}

struct TestUser {
    id: Uuid,
    email: String,
    access_token: String,
}

/// Sends one request and returns status, response headers and the JSON body
/// (`Null` when empty or not JSON).
async fn call_full(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    if let Some(cookie) = cookie {
        req = req.header("Cookie", cookie);
    }
    let req = match body {
        Some(body) => req
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, json)
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    user: &TestUser,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, _, json) = call_full(app, method, uri, Some(&user.access_token), None, body).await;
    (status, json)
}

async fn register_and_login(app: &Router) -> TestUser {
    let email = format!("acl-{}@example.com", Uuid::new_v4());
    let password = "secure_password_123";

    let (status, _, json) = call_full(
        app,
        "POST",
        "/api/v1/auth/register",
        None,
        None,
        Some(json!({"email": email, "password": password, "display_name": "ACL User"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = Uuid::parse_str(json["id"].as_str().unwrap()).unwrap();

    let (status, _, json) = call_full(
        app,
        "POST",
        "/api/v1/auth/login",
        None,
        None,
        Some(json!({"email": email, "password": password})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    TestUser {
        id,
        email,
        access_token: json["access_token"].as_str().unwrap().to_string(),
    }
}

fn id_of(v: &Value) -> Uuid {
    Uuid::parse_str(v["id"].as_str().expect("id in response")).unwrap()
}

async fn create_team(app: &Router, user: &TestUser) -> Uuid {
    let (status, json) = call(
        app,
        "POST",
        "/api/v1/teams",
        user,
        Some(json!({"name": format!("ACL Team {}", Uuid::new_v4())})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    id_of(&json)
}

async fn add_member(app: &Router, caller: &TestUser, team_id: Uuid, user: &TestUser, role: &str) {
    let (status, json) = call(
        app,
        "POST",
        &format!("/api/v1/teams/{team_id}/members"),
        caller,
        Some(json!({"user_id": user.id, "role": role})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "add member: {json}");
}

async fn create_project(app: &Router, user: &TestUser, team_id: Uuid, name: &str) -> Uuid {
    let (status, json) = call(
        app,
        "POST",
        &format!("/api/v1/teams/{team_id}/projects"),
        user,
        Some(json!({"name": name})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    id_of(&json)
}

async fn add_project_member(app: &Router, caller: &TestUser, project_id: Uuid, user: &TestUser) {
    let (status, json) = call(
        app,
        "POST",
        &format!("/api/v1/projects/{project_id}/members"),
        caller,
        Some(json!({"user_id": user.id})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "add project member: {json}");
}

/// The id of the project's first seeded column (Backlog).
async fn first_column(app: &Router, user: &TestUser, project_id: Uuid) -> Uuid {
    let (status, board) = call(
        app,
        "GET",
        &format!("/api/v1/projects/{project_id}/board"),
        user,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    id_of(&board["columns"][0])
}

async fn create_task(app: &Router, user: &TestUser, project_id: Uuid, title: &str) -> Value {
    let column_id = first_column(app, user, project_id).await;
    let (status, json) = call(
        app,
        "POST",
        &format!("/api/v1/projects/{project_id}/tasks"),
        user,
        Some(json!({"title": title, "column_id": column_id})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create task: {json}");
    json
}

// ── Teams ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_cannot_add_a_member_as_owner() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let admin = register_and_login(&app).await;
    let accomplice = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    add_member(&app, &owner, team_id, &admin, "admin").await;

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/teams/{team_id}/members"),
        &admin,
        Some(json!({"user_id": accomplice.id, "role": "owner"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // And they did not end up on the team under any role.
    let (_, members) = call(
        &app,
        "GET",
        &format!("/api/v1/teams/{team_id}/members"),
        &owner,
        None,
    )
    .await;
    assert!(
        members
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["user_id"] != json!(accomplice.id))
    );
}

#[tokio::test]
async fn personal_workspace_does_not_accept_members() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let other = register_and_login(&app).await;

    let (_, teams) = call(&app, "GET", "/api/v1/teams", &owner, None).await;
    let personal = teams
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["is_personal"] == json!(true))
        .expect("personal team");
    let team_id = id_of(personal);

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/teams/{team_id}/members"),
        &owner,
        Some(json!({"user_id": other.id, "role": "member"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn renaming_a_team_to_its_own_name_keeps_the_slug() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let name = format!("Slug Team {}", Uuid::new_v4());
    let (_, created) = call(
        &app,
        "POST",
        "/api/v1/teams",
        &owner,
        Some(json!({"name": name})),
    )
    .await;
    let team_id = id_of(&created);

    let (status, renamed) = call(
        &app,
        "PATCH",
        &format!("/api/v1/teams/{team_id}"),
        &owner,
        Some(json!({"name": name.to_uppercase()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["slug"], created["slug"]);
}

#[tokio::test]
async fn leaving_a_team_clears_project_access_for_a_later_rejoin() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let member = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    add_member(&app, &owner, team_id, &member, "member").await;
    let project_id = create_project(&app, &owner, team_id, "Rejoin").await;
    add_project_member(&app, &owner, project_id, &member).await;

    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/teams/{team_id}/members/{}", member.id),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Back on the team, but the old project grant must not have come back.
    add_member(&app, &owner, team_id, &member, "member").await;
    let (status, _) = call(
        &app,
        "GET",
        &format!("/api/v1/projects/{project_id}"),
        &member,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ── Project-level access ─────────────────────────────────────────────────────

#[tokio::test]
async fn task_refs_only_cover_projects_the_caller_can_see() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let member = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    add_member(&app, &owner, team_id, &member, "member").await;

    let visible = create_project(&app, &owner, team_id, "Visible").await;
    let hidden = create_project(&app, &owner, team_id, "Hidden").await;
    add_project_member(&app, &owner, visible, &member).await;
    create_task(&app, &owner, visible, "visible task").await;
    create_task(&app, &owner, hidden, "hidden task").await;

    let uri = format!("/api/v1/teams/{team_id}/task-refs");
    let titles = |refs: &Value| -> Vec<String> {
        refs.as_array()
            .unwrap()
            .iter()
            .map(|r| r["title"].as_str().unwrap().to_string())
            .collect()
    };

    let (status, refs) = call(&app, "GET", &uri, &member, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&refs), vec!["visible task"]);

    // The owner still sees everything.
    let (_, refs) = call(&app, "GET", &uri, &owner, None).await;
    assert_eq!(titles(&refs).len(), 2);
}

#[tokio::test]
async fn github_project_endpoints_require_project_membership() {
    let app = app_with(true).await;
    let owner = register_and_login(&app).await;
    let member = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    add_member(&app, &owner, team_id, &member, "member").await;
    let project_id = create_project(&app, &owner, team_id, "Private repo").await;

    for path in [
        "issues",
        "prs",
        "branches",
        "pr-template",
        "issue-templates",
    ] {
        let uri = format!("/api/v1/projects/{project_id}/github/{path}");
        let (status, _) = call(&app, "GET", &uri, &member, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "GET {path}");
    }
    for (method, path) in [("PUT", "prs/1/merge"), ("POST", "prs/1/reviews")] {
        let uri = format!("/api/v1/projects/{project_id}/github/{path}");
        let body = json!({"verdict": "approve", "body": ""});
        let (status, _) = call(&app, method, &uri, &member, Some(body)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
    }

    // Once added to the project the same call gets past the access check and
    // fails on the next thing instead (no repository configured).
    add_project_member(&app, &owner, project_id, &member).await;
    let uri = format!("/api/v1/projects/{project_id}/github/issues");
    let (status, _) = call(&app, "GET", &uri, &member, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Task and project validation ──────────────────────────────────────────────

#[tokio::test]
async fn task_rejects_labels_from_another_project_and_outside_assignees() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let outsider = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    let project_id = create_project(&app, &owner, team_id, "Mine").await;
    let other_project = create_project(&app, &owner, team_id, "Other").await;

    let (status, foreign_label) = call(
        &app,
        "POST",
        &format!("/api/v1/projects/{other_project}/labels"),
        &owner,
        Some(json!({"name": "foreign", "color": "#ff0000"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, own_label) = call(
        &app,
        "POST",
        &format!("/api/v1/projects/{project_id}/labels"),
        &owner,
        Some(json!({"name": "own", "color": "#00ff00"})),
    )
    .await;

    let column_id = first_column(&app, &owner, project_id).await;
    let tasks_uri = format!("/api/v1/projects/{project_id}/tasks");

    let (status, _) = call(
        &app,
        "POST",
        &tasks_uri,
        &owner,
        Some(json!({"title": "t", "column_id": column_id, "labels": [foreign_label["id"]]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = call(
        &app,
        "POST",
        &tasks_uri,
        &owner,
        Some(json!({"title": "t", "column_id": column_id, "assignee_id": outsider.id})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The project's own label and a team member are still fine, on create…
    let (status, task) = call(
        &app,
        "POST",
        &tasks_uri,
        &owner,
        Some(json!({
            "title": "t",
            "column_id": column_id,
            "labels": [own_label["id"]],
            "assignee_id": owner.id,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // …while the same bad references are refused on update too.
    let task_uri = format!("/api/v1/tasks/{}", id_of(&task));
    let (status, _) = call(
        &app,
        "PATCH",
        &task_uri,
        &owner,
        Some(json!({"labels": [foreign_label["id"]]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        "PATCH",
        &task_uri,
        &owner,
        Some(json!({"assignee_id": outsider.id})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn link_task_rejects_a_malformed_repository() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    let project_id = create_project(&app, &owner, team_id, "Links").await;
    let task = create_task(&app, &owner, project_id, "link me").await;
    let uri = format!("/api/v1/tasks/{}/link", id_of(&task));

    for repo in [
        "../../user",
        "owner/name/extra",
        "owner/na me",
        "owner/name?x=1",
        "noslash",
    ] {
        let (status, _) = call(
            &app,
            "POST",
            &uri,
            &owner,
            Some(json!({"repo": repo, "issue_number": 1})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "repo {repo:?}");
    }

    let (status, linked) = call(
        &app,
        "POST",
        &uri,
        &owner,
        Some(json!({"repo": "acme/api.v2", "issue_number": 1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(linked["github_repo"], "acme/api.v2");
}

#[tokio::test]
async fn project_update_rejects_foreign_columns_and_bad_repos() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    let project_id = create_project(&app, &owner, team_id, "Mine").await;
    let other_project = create_project(&app, &owner, team_id, "Other").await;
    let foreign_column = first_column(&app, &owner, other_project).await;
    let uri = format!("/api/v1/projects/{project_id}");

    let (status, _) = call(
        &app,
        "PATCH",
        &uri,
        &owner,
        Some(json!({"done_column_id": foreign_column})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = call(
        &app,
        "PATCH",
        &uri,
        &owner,
        Some(json!({"github_repo": "../../orgs/acme"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let own_column = first_column(&app, &owner, project_id).await;
    let (status, updated) = call(
        &app,
        "PATCH",
        &uri,
        &owner,
        Some(json!({"done_column_id": own_column, "github_repo": "acme/api"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["github_repo"], "acme/api");
}

#[tokio::test]
async fn move_reports_whether_the_column_changed() {
    let app = setup_app().await;
    let pool = pool().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    let project_id = create_project(&app, &owner, team_id, "Moves").await;
    let task = create_task(&app, &owner, project_id, "move me").await;
    let task_id = id_of(&task);
    let (_, board) = call(
        &app,
        "GET",
        &format!("/api/v1/projects/{project_id}/board"),
        &owner,
        None,
    )
    .await;
    let backlog = id_of(&board["columns"][0]);
    let in_progress = id_of(&board["columns"][1]);

    // A reorder inside the same column is not a column change…
    let moved = weld_server::services::task::move_task(&pool, task_id, owner.id, backlog, 5.0)
        .await
        .unwrap();
    assert!(!moved.column_changed);

    // …moving to another column is.
    let moved = weld_server::services::task::move_task(&pool, task_id, owner.id, in_progress, 1.0)
        .await
        .unwrap();
    assert!(moved.column_changed);
}

// ── Chat ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn team_admin_cannot_moderate_a_private_channel_they_are_not_in() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let admin = register_and_login(&app).await;
    let member = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    add_member(&app, &owner, team_id, &admin, "admin").await;
    add_member(&app, &owner, team_id, &member, "member").await;

    let (status, channel) = call(
        &app,
        "POST",
        &format!("/api/v1/teams/{team_id}/channels"),
        &member,
        Some(json!({"name": "secret", "is_private": true})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let channel_id = id_of(&channel);

    let (status, message) = call(
        &app,
        "POST",
        &format!("/api/v1/channels/{channel_id}/messages"),
        &member,
        Some(json!({"content": "between us"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let channel_uri = format!("/api/v1/channels/{channel_id}");
    let (status, _) = call(
        &app,
        "PATCH",
        &channel_uri,
        &admin,
        Some(json!({"topic": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&app, "DELETE", &channel_uri, &admin, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/messages/{}", id_of(&message)),
        &admin,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Still there for the people actually in it.
    let (status, _) = call(&app, "GET", &channel_uri, &member, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn message_list_reports_thread_reply_counts() {
    let app = setup_app().await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    let (_, channel) = call(
        &app,
        "POST",
        &format!("/api/v1/teams/{team_id}/channels"),
        &owner,
        Some(json!({"name": "threads"})),
    )
    .await;
    let messages_uri = format!("/api/v1/channels/{}/messages", id_of(&channel));

    let (_, parent) = call(
        &app,
        "POST",
        &messages_uri,
        &owner,
        Some(json!({"content": "parent"})),
    )
    .await;
    let (_, _lonely) = call(
        &app,
        "POST",
        &messages_uri,
        &owner,
        Some(json!({"content": "lonely"})),
    )
    .await;
    let mut reply_ids = Vec::new();
    for n in 0..3 {
        let (status, reply) = call(
            &app,
            "POST",
            &messages_uri,
            &owner,
            Some(json!({"content": format!("reply {n}"), "thread_parent_id": parent["id"]})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        reply_ids.push(id_of(&reply));
    }
    // A deleted reply no longer counts.
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/messages/{}", reply_ids[0]),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, page) = call(&app, "GET", &messages_uri, &owner, None).await;
    let count_of = |content: &str| {
        page["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["content"] == content)
            .unwrap()["reply_count"]
            .clone()
    };
    assert_eq!(count_of("parent"), json!(2));
    assert_eq!(count_of("lonely"), json!(0));
}

// ── Auth ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn email_is_case_insensitive_for_login_and_uniqueness() {
    let app = setup_app().await;
    let user = register_and_login(&app).await;
    let shouted = user.email.to_uppercase();

    let (status, _, _) = call_full(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        None,
        Some(json!({"email": shouted, "password": "secure_password_123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _, _) = call_full(
        &app,
        "POST",
        "/api/v1/auth/register",
        None,
        None,
        Some(json!({"email": shouted, "password": "secure_password_123", "display_name": "Twin"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn seeded_service_account_login_is_a_plain_401() {
    let app = setup_app().await;
    // The GitHub bot has an empty password hash; that used to surface as a 500.
    let (status, _, _) = call_full(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        None,
        Some(json!({"email": "github-bot@weld.internal", "password": "whatever-password"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn repeated_failed_logins_are_throttled_per_account() {
    let app = setup_app().await;
    let user = register_and_login(&app).await;
    let attempt = |password: &'static str| {
        call_full(
            &app,
            "POST",
            "/api/v1/auth/login",
            None,
            None,
            Some(json!({"email": user.email, "password": password})),
        )
    };

    for _ in 0..10 {
        let (status, _, _) = attempt("wrong-password-123").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    // The allowance is spent: even the right password has to wait now.
    let (status, _, _) = attempt("wrong-password-123").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _, _) = attempt("secure_password_123").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn logout_expires_the_refresh_cookie() {
    let app = setup_app().await;
    let user = register_and_login(&app).await;
    let (_, headers, _) = call_full(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        None,
        Some(json!({"email": user.email, "password": "secure_password_123"})),
    )
    .await;
    let cookie = headers["set-cookie"].to_str().unwrap();
    let cookie = cookie.split(';').next().unwrap().to_string();

    let (status, headers, _) = call_full(
        &app,
        "POST",
        "/api/v1/auth/logout",
        None,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cleared = headers["set-cookie"].to_str().unwrap();
    assert!(cleared.starts_with("refresh_token=;"), "{cleared}");
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
}

// ── GitHub callbacks ─────────────────────────────────────────────────────────

/// Pulls `state=…` out of a URL returned by the install/oauth URL endpoints.
fn state_param(url: &str) -> String {
    url.split("state=").nth(1).expect("state param").to_string()
}

/// The `name=value` part of a Set-Cookie header, ready to send back.
fn cookie_pair(headers: &HeaderMap) -> String {
    headers["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn setup_callback_requires_the_browser_that_started_the_install() {
    let app = app_with(true).await;
    let owner = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;

    let (status, headers, body) = call_full(
        &app,
        "GET",
        &format!("/api/v1/teams/{team_id}/github/install-url"),
        Some(&owner.access_token),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let state = state_param(body["url"].as_str().unwrap());
    let cookie = cookie_pair(&headers);
    assert!(cookie.starts_with("gh_state_install="));
    let uri =
        format!("/api/v1/github/setup?installation_id=12345&setup_action=install&state={state}");

    // A valid, signed state on its own is not enough: this is the request an
    // attacker replays with someone else's installation id.
    let (status, _, _) = call_full(&app, "GET", &uri, None, None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _, _) = call_full(
        &app,
        "GET",
        &uri,
        None,
        Some("gh_state_install=forged"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The initiating browser gets past the binding. It has no linked GitHub
    // account to prove it owns installation 12345 with, so it is sent back to
    // link one — and no connection is stored.
    let (status, headers, _) = call_full(&app, "GET", &uri, None, Some(&cookie), None).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = headers["location"].to_str().unwrap();
    assert!(
        location.ends_with("/settings?github_error=link_required"),
        "{location}"
    );

    let (status, _) = call(
        &app,
        "GET",
        &format!("/api/v1/teams/{team_id}/github"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn setup_callback_rechecks_the_initiators_role() {
    let app = app_with(true).await;
    let owner = register_and_login(&app).await;
    let admin = register_and_login(&app).await;
    let team_id = create_team(&app, &owner).await;
    add_member(&app, &owner, team_id, &admin, "admin").await;

    let (_, headers, body) = call_full(
        &app,
        "GET",
        &format!("/api/v1/teams/{team_id}/github/install-url"),
        Some(&admin.access_token),
        None,
        None,
    )
    .await;
    let state = state_param(body["url"].as_str().unwrap());
    let cookie = cookie_pair(&headers);

    // Removed from the team before finishing the install.
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/teams/{team_id}/members/{}", admin.id),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let uri = format!("/api/v1/github/setup?installation_id=12345&state={state}");
    let (status, _, _) = call_full(&app, "GET", &uri, None, Some(&cookie), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn oauth_callback_rejects_a_state_not_bound_to_the_browser() {
    use weld_server::services::github::helpers::sign_state_token;

    let app = app_with(true).await;
    let attacker = register_and_login(&app).await;

    // A correctly signed state for the attacker's account, with no browser
    // binding — what a victim would be tricked into carrying to the callback.
    dotenvy::dotenv().ok();
    let secret = std::env::var("JWT_SECRET").unwrap();
    let unbound = sign_state_token(&secret, attacker.id, "oauth").unwrap();
    let uri = format!("/api/v1/github/oauth/callback?code=victims-code&state={unbound}");
    let (status, _, _) = call_full(&app, "GET", &uri, None, None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // A real, bound state still fails in a browser that doesn't hold its cookie.
    let (status, headers, body) = call_full(
        &app,
        "GET",
        "/api/v1/github/oauth/url",
        Some(&attacker.access_token),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(cookie_pair(&headers).starts_with("gh_state_oauth="));
    let bound = state_param(body["url"].as_str().unwrap());
    let uri = format!("/api/v1/github/oauth/callback?code=victims-code&state={bound}");
    let (status, _, _) = call_full(&app, "GET", &uri, None, None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
