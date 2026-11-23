//! Integration tests for the realtime WebSocket hub.
//!
//! These spin up the full router on a real local TCP port (rather than going
//! through `tower::oneshot`) so that the WebSocket upgrade handshake actually
//! runs end-to-end.

use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;
use weld_server::realtime::Hub;

async fn build_app() -> Router {
    dotenvy::dotenv().ok();
    let config = weld_server::config::Config::from_env();
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("connect db");
    let state = weld_server::state::AppState {
        db: pool,
        jwt_secret: config.jwt_secret,
        secure_cookies: false,
        hub: Arc::new(Hub::new()),
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

async fn spawn_server() -> SocketAddr {
    let app = build_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

async fn http_post(
    client: &reqwest::Client,
    addr: SocketAddr,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (reqwest::StatusCode, Value) {
    let mut req = client
        .post(format!("http://{addr}{path}"))
        .json(&body)
        .header("Content-Type", "application/json");
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status();
    let text = resp.text().await.unwrap();
    let value: Value = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or(Value::String(text))
    };
    (status, value)
}

struct AuthedUser {
    id: Uuid,
    access_token: String,
}

async fn register_login(client: &reqwest::Client, addr: SocketAddr) -> AuthedUser {
    let email = format!("rt-{}@example.com", Uuid::new_v4());
    let password = "secure_password_123";
    let (status, body) = http_post(
        client,
        addr,
        "/api/v1/auth/register",
        None,
        json!({"email": email, "password": password, "display_name": "RT User"}),
    )
    .await;
    assert!(status.is_success(), "register: {status} {body}");
    let id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let (status, body) = http_post(
        client,
        addr,
        "/api/v1/auth/login",
        None,
        json!({"email": email, "password": password}),
    )
    .await;
    assert!(status.is_success(), "login: {status}");
    AuthedUser {
        id,
        access_token: body["access_token"].as_str().unwrap().to_string(),
    }
}

async fn create_team(client: &reqwest::Client, addr: SocketAddr, user: &AuthedUser) -> Uuid {
    let (status, body) = http_post(
        client,
        addr,
        "/api/v1/teams",
        Some(&user.access_token),
        json!({"name": format!("Realtime Team {}", Uuid::new_v4())}),
    )
    .await;
    assert!(status.is_success(), "team: {status} {body}");
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

async fn create_project(
    client: &reqwest::Client,
    addr: SocketAddr,
    user: &AuthedUser,
    team_id: Uuid,
) -> Uuid {
    let (status, body) = http_post(
        client,
        addr,
        &format!("/api/v1/teams/{team_id}/projects"),
        Some(&user.access_token),
        json!({"name": "Realtime Project"}),
    )
    .await;
    assert!(status.is_success(), "project: {status} {body}");
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

async fn create_column(
    client: &reqwest::Client,
    addr: SocketAddr,
    user: &AuthedUser,
    project_id: Uuid,
) -> Uuid {
    let (status, body) = http_post(
        client,
        addr,
        &format!("/api/v1/projects/{project_id}/columns"),
        Some(&user.access_token),
        json!({"name": "Todo"}),
    )
    .await;
    assert!(status.is_success(), "column: {status} {body}");
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

/// Reads up to one event matching `predicate`, with a short timeout.
async fn next_event_matching<F>(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    predicate: F,
) -> Option<Value>
where
    F: Fn(&Value) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline - tokio::time::Instant::now();
        let Ok(maybe) = tokio::time::timeout(remaining, ws.next()).await else {
            return None;
        };
        match maybe {
            Some(Ok(Message::Text(text))) => {
                if let Ok(v) = serde_json::from_str::<Value>(&text)
                    && predicate(&v)
                {
                    return Some(v);
                }
            }
            Some(Ok(_)) => continue,
            _ => return None,
        }
    }
    None
}

#[tokio::test]
async fn ws_rejects_unauthenticated_connection() {
    let addr = spawn_server().await;
    let url = format!("ws://{addr}/api/v1/ws");
    let result = connect_async(url).await;
    if let Ok((mut ws, _)) = result {
        // No token in query and no auth message: server should close 4401.
        let next = tokio::time::timeout(Duration::from_secs(8), ws.next()).await;
        match next {
            Ok(Some(Ok(Message::Close(Some(frame))))) => {
                assert_eq!(u16::from(frame.code), 4401);
            }
            Ok(Some(Ok(Message::Close(None)))) => { /* also acceptable */ }
            Ok(None) => { /* connection dropped — also acceptable */ }
            other => panic!("expected close frame, got {other:?}"),
        }
    }
    // If the upgrade itself failed that's also fine — both outcomes mean
    // the unauthenticated client cannot stay connected.
}

#[tokio::test]
async fn task_create_broadcasts_to_subscribed_client() {
    let addr = spawn_server().await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    let user = register_login(&client, addr).await;
    let team_id = create_team(&client, addr, &user).await;
    let project_id = create_project(&client, addr, &user, team_id).await;
    let column_id = create_column(&client, addr, &user, project_id).await;

    // Connect WS with token query param, subscribe to project room.
    let url = format!("ws://{addr}/api/v1/ws?token={}", user.access_token);
    let (mut ws, _) = connect_async(url).await.expect("ws connect");
    let sub = json!({"type": "ws.subscribe", "rooms": [format!("project:{project_id}")]});
    ws.send(Message::Text(sub.to_string().into()))
        .await
        .unwrap();

    // Wait for the subscribed ack so we know the registry has us before the
    // mutation broadcasts.
    let ack = next_event_matching(&mut ws, |v| v["type"] == "ws.subscribed")
        .await
        .expect("subscribed ack");
    assert_eq!(ack["rooms"][0], format!("project:{project_id}"));

    // Trigger a mutation via REST.
    let (status, _body) = http_post(
        &client,
        addr,
        &format!("/api/v1/projects/{project_id}/tasks"),
        Some(&user.access_token),
        json!({"title": "Hello realtime", "column_id": column_id}),
    )
    .await;
    assert!(status.is_success());

    let evt = next_event_matching(&mut ws, |v| v["type"] == "task.created")
        .await
        .expect("task.created event");
    assert_eq!(evt["room"], format!("project:{project_id}"));
    assert_eq!(evt["payload"]["title"], "Hello realtime");
    assert_eq!(evt["payload"]["project_id"], project_id.to_string());

    let _ = ws.close(None).await;
    // Suppress "unused" on user.id without changing test semantics.
    let _ = user.id;
}

async fn create_channel(
    client: &reqwest::Client,
    addr: SocketAddr,
    user: &AuthedUser,
    team_id: Uuid,
    name: &str,
) -> Uuid {
    let (status, body) = http_post(
        client,
        addr,
        &format!("/api/v1/teams/{team_id}/channels"),
        Some(&user.access_token),
        json!({"name": name}),
    )
    .await;
    assert!(status.is_success(), "channel: {status} {body}");
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn chat_message_new_broadcasts_to_subscribed_client() {
    let addr = spawn_server().await;
    let client = reqwest::Client::builder().build().unwrap();
    let user = register_login(&client, addr).await;
    let team_id = create_team(&client, addr, &user).await;
    let channel_id = create_channel(&client, addr, &user, team_id, "general").await;

    let url = format!("ws://{addr}/api/v1/ws?token={}", user.access_token);
    let (mut ws, _) = connect_async(url).await.expect("ws connect");
    let sub = json!({"type": "ws.subscribe", "rooms": [format!("channel:{channel_id}")]});
    ws.send(Message::Text(sub.to_string().into()))
        .await
        .unwrap();
    let _ack = next_event_matching(&mut ws, |v| v["type"] == "ws.subscribed")
        .await
        .expect("subscribed ack");

    let (status, _body) = http_post(
        &client,
        addr,
        &format!("/api/v1/channels/{channel_id}/messages"),
        Some(&user.access_token),
        json!({"content": "hi realtime"}),
    )
    .await;
    assert!(status.is_success());

    let evt = next_event_matching(&mut ws, |v| v["type"] == "chat.message.new")
        .await
        .expect("chat.message.new event");
    assert_eq!(evt["room"], format!("channel:{channel_id}"));
    assert_eq!(evt["payload"]["content"], "hi realtime");
    let _ = ws.close(None).await;
    let _ = user.id;
}

#[tokio::test]
async fn chat_typing_rebroadcasts_as_user_typing() {
    let addr = spawn_server().await;
    let client = reqwest::Client::builder().build().unwrap();
    let owner = register_login(&client, addr).await;
    let watcher = register_login(&client, addr).await;
    let team_id = create_team(&client, addr, &owner).await;
    // add watcher as a member
    let (status, _) = http_post(
        &client,
        addr,
        &format!("/api/v1/teams/{team_id}/members"),
        Some(&owner.access_token),
        json!({"user_id": watcher.id, "role": "member"}),
    )
    .await;
    assert!(status.is_success());
    let channel_id = create_channel(&client, addr, &owner, team_id, "general").await;

    // watcher subscribes
    let url = format!("ws://{addr}/api/v1/ws?token={}", watcher.access_token);
    let (mut watcher_ws, _) = connect_async(url).await.unwrap();
    let sub = json!({"type": "ws.subscribe", "rooms": [format!("channel:{channel_id}")]});
    watcher_ws
        .send(Message::Text(sub.to_string().into()))
        .await
        .unwrap();
    let _ = next_event_matching(&mut watcher_ws, |v| v["type"] == "ws.subscribed")
        .await
        .expect("subscribed ack");

    // owner sends a typing event
    let url = format!("ws://{addr}/api/v1/ws?token={}", owner.access_token);
    let (mut owner_ws, _) = connect_async(url).await.unwrap();
    let typing = json!({"type": "chat.typing", "channel_id": channel_id});
    owner_ws
        .send(Message::Text(typing.to_string().into()))
        .await
        .unwrap();

    let evt = next_event_matching(&mut watcher_ws, |v| v["type"] == "chat.user.typing")
        .await
        .expect("chat.user.typing");
    assert_eq!(evt["payload"]["channel_id"], channel_id.to_string());
    assert_eq!(evt["payload"]["user_id"], owner.id.to_string());
    let _ = owner_ws.close(None).await;
    let _ = watcher_ws.close(None).await;
}

#[tokio::test]
async fn ws_subscribe_with_since_replays_missed_messages() {
    let addr = spawn_server().await;
    let client = reqwest::Client::builder().build().unwrap();
    let user = register_login(&client, addr).await;
    let team_id = create_team(&client, addr, &user).await;
    let channel_id = create_channel(&client, addr, &user, team_id, "general").await;

    // Post one message *before* connecting — must NOT be replayed.
    let (status, before_msg) = http_post(
        &client,
        addr,
        &format!("/api/v1/channels/{channel_id}/messages"),
        Some(&user.access_token),
        json!({"content": "before cursor"}),
    )
    .await;
    assert!(status.is_success());
    let cursor = before_msg["created_at"].as_str().unwrap().to_string();

    // Post a second message *after* the cursor — must be replayed on subscribe.
    let (status, _) = http_post(
        &client,
        addr,
        &format!("/api/v1/channels/{channel_id}/messages"),
        Some(&user.access_token),
        json!({"content": "after cursor"}),
    )
    .await;
    assert!(status.is_success());

    // Now connect with `since = cursor` and expect the second message replayed.
    let url = format!("ws://{addr}/api/v1/ws?token={}", user.access_token);
    let (mut ws, _) = connect_async(url).await.unwrap();
    let sub = json!({
        "type": "ws.subscribe",
        "rooms": [format!("channel:{channel_id}")],
        "since": cursor,
    });
    ws.send(Message::Text(sub.to_string().into()))
        .await
        .unwrap();

    let evt = next_event_matching(&mut ws, |v| {
        v["type"] == "chat.message.new" && v["payload"]["content"] == "after cursor"
    })
    .await
    .expect("backfilled chat.message.new");
    assert_eq!(evt["room"], format!("channel:{channel_id}"));

    // The before-cursor message must not be replayed.
    let stray = next_event_matching(&mut ws, |v| {
        v["type"] == "chat.message.new" && v["payload"]["content"] == "before cursor"
    })
    .await;
    assert!(stray.is_none(), "messages older than cursor leaked through");

    let _ = ws.close(None).await;
}

#[tokio::test]
async fn ws_connect_broadcasts_presence_online_to_team() {
    let addr = spawn_server().await;
    let client = reqwest::Client::builder().build().unwrap();
    let owner = register_login(&client, addr).await;
    let watcher = register_login(&client, addr).await;
    let team_id = create_team(&client, addr, &owner).await;
    let (status, _) = http_post(
        &client,
        addr,
        &format!("/api/v1/teams/{team_id}/members"),
        Some(&owner.access_token),
        json!({"user_id": watcher.id, "role": "member"}),
    )
    .await;
    assert!(status.is_success());

    // Watcher connects first and subscribes to the team room.
    let url = format!("ws://{addr}/api/v1/ws?token={}", watcher.access_token);
    let (mut watcher_ws, _) = connect_async(url).await.unwrap();
    let sub = json!({"type": "ws.subscribe", "rooms": [format!("team:{team_id}")]});
    watcher_ws
        .send(Message::Text(sub.to_string().into()))
        .await
        .unwrap();
    let _ack = next_event_matching(&mut watcher_ws, |v| v["type"] == "ws.subscribed")
        .await
        .expect("subscribed ack");

    // Owner connects → triggers presence.online to the team room.
    let url = format!("ws://{addr}/api/v1/ws?token={}", owner.access_token);
    let (mut owner_ws, _) = connect_async(url).await.unwrap();

    let evt = next_event_matching(&mut watcher_ws, |v| v["type"] == "presence.online")
        .await
        .expect("presence.online");
    assert_eq!(evt["room"], format!("team:{team_id}"));
    assert_eq!(evt["payload"]["user_id"], owner.id.to_string());

    let _ = owner_ws.close(None).await;
    let _ = watcher_ws.close(None).await;
}

#[tokio::test]
async fn ws_subscribe_to_unauthorized_project_is_rejected() {
    let addr = spawn_server().await;
    let client = reqwest::Client::builder().build().unwrap();
    // Outsider account.
    let outsider = register_login(&client, addr).await;
    // Owner account creates project, outsider tries to subscribe.
    let owner = register_login(&client, addr).await;
    let team_id = create_team(&client, addr, &owner).await;
    let project_id = create_project(&client, addr, &owner, team_id).await;

    let url = format!("ws://{addr}/api/v1/ws?token={}", outsider.access_token);
    let (mut ws, _) = connect_async(url).await.unwrap();
    let sub = json!({"type": "ws.subscribe", "rooms": [format!("project:{project_id}")]});
    ws.send(Message::Text(sub.to_string().into()))
        .await
        .unwrap();

    let err = next_event_matching(&mut ws, |v| v["type"] == "ws.error")
        .await
        .expect("ws.error for unauthorized room");
    assert_eq!(err["room"], format!("project:{project_id}"));
    let _ = ws.close(None).await;
}

#[tokio::test]
async fn ws_project_room_requires_project_membership_not_just_team() {
    let addr = spawn_server().await;
    let client = reqwest::Client::builder().build().unwrap();
    let owner = register_login(&client, addr).await;
    let member = register_login(&client, addr).await;
    let team_id = create_team(&client, addr, &owner).await;
    let project_id = create_project(&client, addr, &owner, team_id).await;
    // On the team, but never added to the project.
    let (status, _) = http_post(
        &client,
        addr,
        &format!("/api/v1/teams/{team_id}/members"),
        Some(&owner.access_token),
        json!({"user_id": member.id, "role": "member"}),
    )
    .await;
    assert!(status.is_success());

    let url = format!("ws://{addr}/api/v1/ws?token={}", member.access_token);
    let (mut ws, _) = connect_async(url).await.unwrap();
    let sub = json!({"type": "ws.subscribe", "rooms": [format!("project:{project_id}")]});
    ws.send(Message::Text(sub.to_string().into()))
        .await
        .unwrap();

    let err = next_event_matching(&mut ws, |v| v["type"] == "ws.error")
        .await
        .expect("ws.error for a project the member was not added to");
    assert_eq!(err["room"], format!("project:{project_id}"));
    let _ = ws.close(None).await;
}

#[tokio::test]
async fn ws_subscription_is_revoked_when_removed_from_private_channel() {
    let addr = spawn_server().await;
    let client = reqwest::Client::builder().build().unwrap();
    let owner = register_login(&client, addr).await;
    let member = register_login(&client, addr).await;
    let team_id = create_team(&client, addr, &owner).await;
    let (status, _) = http_post(
        &client,
        addr,
        &format!("/api/v1/teams/{team_id}/members"),
        Some(&owner.access_token),
        json!({"user_id": member.id, "role": "member"}),
    )
    .await;
    assert!(status.is_success());

    let (status, channel) = http_post(
        &client,
        addr,
        &format!("/api/v1/teams/{team_id}/channels"),
        Some(&owner.access_token),
        json!({"name": "private-room", "is_private": true, "members": [member.id]}),
    )
    .await;
    assert!(status.is_success(), "channel: {status} {channel}");
    let channel_id = channel["id"].as_str().unwrap().to_string();

    let url = format!("ws://{addr}/api/v1/ws?token={}", member.access_token);
    let (mut ws, _) = connect_async(url).await.unwrap();
    let sub = json!({"type": "ws.subscribe", "rooms": [format!("channel:{channel_id}")]});
    ws.send(Message::Text(sub.to_string().into()))
        .await
        .unwrap();
    next_event_matching(&mut ws, |v| v["type"] == "ws.subscribed")
        .await
        .expect("subscribed while a member");

    // Sanity: while a member, messages arrive.
    let messages_path = format!("/api/v1/channels/{channel_id}/messages");
    let (status, _) = http_post(
        &client,
        addr,
        &messages_path,
        Some(&owner.access_token),
        json!({"content": "before removal"}),
    )
    .await;
    assert!(status.is_success());
    next_event_matching(&mut ws, |v| v["type"] == "chat.message.new")
        .await
        .expect("message delivered while a member");

    // Removed from the channel with the socket still open.
    let resp = client
        .delete(format!(
            "http://{addr}/api/v1/channels/{channel_id}/members/{}",
            member.id
        ))
        .bearer_auth(&owner.access_token)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let (status, _) = http_post(
        &client,
        addr,
        &messages_path,
        Some(&owner.access_token),
        json!({"content": "after removal"}),
    )
    .await;
    assert!(status.is_success());
    let leaked = next_event_matching(&mut ws, |v| v["type"] == "chat.message.new").await;
    assert!(
        leaked.is_none(),
        "removed member still received: {leaked:?}"
    );
    let _ = ws.close(None).await;
}
