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
        json!({"name": "Realtime Team"}),
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
