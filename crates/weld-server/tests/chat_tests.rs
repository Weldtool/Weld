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
        master_encryption_key: None,
        frontend_url: String::from("http://localhost:5173"),
    };
    weld_server::routes::router().with_state(state)
}

struct TestUser {
    id: Uuid,
    access_token: String,
}

async fn register_and_login(app: &Router, display_name: &str) -> TestUser {
    let email = format!("chat-{}@example.com", Uuid::new_v4());
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
                        "display_name": display_name,
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

async fn add_team_member(
    app: &Router,
    caller: &TestUser,
    team_id: Uuid,
    new_user_id: Uuid,
    role: &str,
) {
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
        "add member failed: status={}",
        resp.status()
    );
}

async fn create_channel(app: &Router, user: &TestUser, team_id: Uuid, body_json: Value) -> Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/channels", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", user.access_token))
                .body(Body::from(body_json.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "create channel failed: {}",
        resp.status()
    );
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn post_message(
    app: &Router,
    user: &TestUser,
    channel_id: Uuid,
    content: &str,
    parent: Option<Uuid>,
) -> (StatusCode, Value) {
    let mut body = json!({"content": content});
    if let Some(p) = parent {
        body["thread_parent_id"] = json!(p);
    }
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/channels/{}/messages", channel_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", user.access_token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
    (status, v)
}

async fn list_messages(
    app: &Router,
    user: &TestUser,
    channel_id: Uuid,
    before: Option<Uuid>,
    limit: Option<i64>,
) -> Value {
    let mut uri = format!("/api/v1/channels/{}/messages", channel_id);
    let mut sep = '?';
    if let Some(b) = before {
        uri.push(sep);
        uri.push_str(&format!("before={}", b));
        sep = '&';
    }
    if let Some(l) = limit {
        uri.push(sep);
        uri.push_str(&format!("limit={}", l));
    }
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
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
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn create_and_list_public_channel_visible_to_members() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let team_id = create_team(&app, &owner, "ChatTeam").await;

    let ch = create_channel(
        &app,
        &owner,
        team_id,
        json!({"name": "general", "topic": "main"}),
    )
    .await;
    assert_eq!(ch["name"], "general");
    assert_eq!(ch["is_private"], false);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/teams/{}/channels", team_id))
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
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn duplicate_channel_name_returns_409() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let team_id = create_team(&app, &owner, "ChatDup").await;

    let _ = create_channel(&app, &owner, team_id, json!({"name": "general"})).await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/teams/{}/channels", team_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(json!({"name": "GENERAL"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn private_channel_blocks_non_member() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let other = register_and_login(&app, "Other").await;
    let team_id = create_team(&app, &owner, "ChatPriv").await;
    add_team_member(&app, &owner, team_id, other.id, "member").await;

    let ch = create_channel(
        &app,
        &owner,
        team_id,
        json!({"name": "secret", "is_private": true}),
    )
    .await;
    let channel_id = Uuid::parse_str(ch["id"].as_str().unwrap()).unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/channels/{}", channel_id))
                .header("Authorization", format!("Bearer {}", other.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // owner adds them, now they can access
    let add = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/channels/{}/members", channel_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::from(json!({"user_id": other.id}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add.status(), StatusCode::CREATED);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/channels/{}", channel_id))
                .header("Authorization", format!("Bearer {}", other.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_and_paginate_messages_oldest_via_cursor() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let team_id = create_team(&app, &owner, "ChatPage").await;
    let ch = create_channel(&app, &owner, team_id, json!({"name": "general"})).await;
    let channel_id = Uuid::parse_str(ch["id"].as_str().unwrap()).unwrap();

    for i in 0..5 {
        let (status, _) = post_message(&app, &owner, channel_id, &format!("msg {i}"), None).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let page1 = list_messages(&app, &owner, channel_id, None, Some(2)).await;
    let msgs = page1["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
    // newest first: msg 4 then msg 3
    assert_eq!(msgs[0]["content"], "msg 4");
    assert_eq!(msgs[1]["content"], "msg 3");
    let next_before = page1["next_before"]
        .as_str()
        .map(|s| Uuid::parse_str(s).unwrap());
    assert!(next_before.is_some());

    let page2 = list_messages(&app, &owner, channel_id, next_before, Some(2)).await;
    let msgs = page2["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["content"], "msg 2");
    assert_eq!(msgs[1]["content"], "msg 1");
}

#[tokio::test]
async fn thread_replies_excluded_from_top_level_list_and_returned_by_thread_endpoint() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let team_id = create_team(&app, &owner, "ChatThread").await;
    let ch = create_channel(&app, &owner, team_id, json!({"name": "general"})).await;
    let channel_id = Uuid::parse_str(ch["id"].as_str().unwrap()).unwrap();

    let (_, parent) = post_message(&app, &owner, channel_id, "parent", None).await;
    let parent_id = Uuid::parse_str(parent["id"].as_str().unwrap()).unwrap();
    let (_, _r1) = post_message(&app, &owner, channel_id, "reply 1", Some(parent_id)).await;
    let (_, _r2) = post_message(&app, &owner, channel_id, "reply 2", Some(parent_id)).await;

    let page = list_messages(&app, &owner, channel_id, None, None).await;
    let msgs = page["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1, "replies should not appear in top-level list");

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/messages/{}/thread", parent_id))
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
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["replies"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn edit_only_by_author_delete_by_author_or_admin() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let author = register_and_login(&app, "Author").await;
    let other = register_and_login(&app, "Other").await;
    let team_id = create_team(&app, &owner, "ChatEdit").await;
    add_team_member(&app, &owner, team_id, author.id, "member").await;
    add_team_member(&app, &owner, team_id, other.id, "member").await;

    let ch = create_channel(&app, &owner, team_id, json!({"name": "general"})).await;
    let channel_id = Uuid::parse_str(ch["id"].as_str().unwrap()).unwrap();

    let (_, msg) = post_message(&app, &author, channel_id, "hello", None).await;
    let msg_id = Uuid::parse_str(msg["id"].as_str().unwrap()).unwrap();

    // other cannot edit
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/messages/{}", msg_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", other.access_token))
                .body(Body::from(json!({"content": "evil"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // author can edit
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/messages/{}", msg_id))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", author.access_token))
                .body(Body::from(json!({"content": "hi"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // other (member, not admin) cannot delete
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/messages/{}", msg_id))
                .header("Authorization", format!("Bearer {}", other.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // owner (Owner role) can delete someone else's message
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/messages/{}", msg_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn mention_parsing_resolves_team_members_and_skips_code() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let alice = register_and_login(&app, "Alice").await;
    let bob = register_and_login(&app, "Bob").await;
    let team_id = create_team(&app, &owner, "MentionTeam").await;
    add_team_member(&app, &owner, team_id, alice.id, "member").await;
    add_team_member(&app, &owner, team_id, bob.id, "member").await;

    let ch = create_channel(&app, &owner, team_id, json!({"name": "general"})).await;
    let channel_id = Uuid::parse_str(ch["id"].as_str().unwrap()).unwrap();

    let content = "hi @alice — `@bob` should not count\n```\n@bob\n```\nbut here @bob does";
    let (status, msg) = post_message(&app, &owner, channel_id, content, None).await;
    assert_eq!(status, StatusCode::CREATED);
    let mentions = msg["mentions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| Uuid::parse_str(v.as_str().unwrap()).unwrap())
        .collect::<Vec<_>>();
    assert!(mentions.contains(&alice.id));
    assert!(mentions.contains(&bob.id));
    assert_eq!(mentions.len(), 2, "no duplicates");
}

#[tokio::test]
async fn archive_channel_hides_it_from_listings() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let team_id = create_team(&app, &owner, "ChatArchive").await;
    let ch = create_channel(&app, &owner, team_id, json!({"name": "old"})).await;
    let channel_id = Uuid::parse_str(ch["id"].as_str().unwrap()).unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/channels/{}", channel_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // get returns 404
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/channels/{}", channel_id))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // and a fresh channel with the same name now succeeds (slug freed)
    let _ = create_channel(&app, &owner, team_id, json!({"name": "old"})).await;
}

#[tokio::test]
async fn missing_channel_returns_404() {
    let app = setup_app().await;
    let owner = register_and_login(&app, "Owner").await;
    let _ = create_team(&app, &owner, "ChatMissing").await;
    let bogus = Uuid::new_v4();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/channels/{}", bogus))
                .header("Authorization", format!("Bearer {}", owner.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
