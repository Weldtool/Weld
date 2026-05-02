use crate::middleware::auth::Claims;
use crate::realtime::events::{ClientMessage, ServerAck};
use crate::realtime::hub::{ClientHandle, ConnId};
use crate::realtime::{Hub, RoomKey};
use crate::services::team::helpers::require_team_role;
use crate::state::AppState;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use sqlx::PgPool;
use std::str::FromStr;
use tokio::sync::mpsc;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

const CLOSE_UNAUTHORIZED: u16 = 4401;

pub async fn ws_upgrade(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let initial_token = query.token;
    ws.on_upgrade(move |socket| handle_socket(socket, state, initial_token))
}

async fn handle_socket(socket: WebSocket, state: AppState, initial_token: Option<String>) {
    let (mut sink, mut stream) = socket.split();

    // Authenticate via ?token= or the first inbound message of type "auth".
    let user_id = match resolve_user_id(&mut stream, initial_token, &state.jwt_secret).await {
        Some(uid) => uid,
        None => {
            let _ = sink
                .send(Message::Close(Some(CloseFrame {
                    code: CLOSE_UNAUTHORIZED,
                    reason: "unauthorized".into(),
                })))
                .await;
            return;
        }
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let conn_id = state.hub.register(ClientHandle {
        user_id,
        sender: tx,
    });

    // Pump outbound messages from the hub channel to the socket.
    let writer = tokio::spawn(async move {
        while let Some(payload) = rx.recv().await {
            if sink.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // Handle inbound messages.
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(text) => {
                handle_client_message(&text, conn_id, user_id, &state).await;
            }
            Message::Binary(_) => { /* ignore */ }
            Message::Ping(_) | Message::Pong(_) => { /* axum handles pings */ }
            Message::Close(_) => break,
        }
    }

    state.hub.unregister(conn_id);
    writer.abort();
}

async fn resolve_user_id(
    stream: &mut futures_util::stream::SplitStream<WebSocket>,
    query_token: Option<String>,
    jwt_secret: &str,
) -> Option<Uuid> {
    if let Some(token) = query_token
        && let Some(uid) = decode_user_id(&token, jwt_secret)
    {
        return Some(uid);
    }

    // Fall back to the first inbound text frame: must be `{ "type": "auth", "token": "..." }`.
    // Bounded so we don't hold idle slots for unauthenticated clients.
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
        .await
        .ok()??
        .ok()?;
    let Message::Text(text) = msg else {
        return None;
    };
    let parsed: ClientMessage = serde_json::from_str(&text).ok()?;
    let ClientMessage::Auth { token } = parsed else {
        return None;
    };
    decode_user_id(&token, jwt_secret)
}

fn decode_user_id(token: &str, jwt_secret: &str) -> Option<Uuid> {
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_ref()),
        &jsonwebtoken::Validation::default(),
    )
    .ok()?;
    Uuid::parse_str(&data.claims.sub).ok()
}

async fn handle_client_message(text: &str, conn_id: ConnId, user_id: Uuid, state: &AppState) {
    let parsed: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(err) => {
            send_ack(
                &state.hub,
                conn_id,
                ServerAck::Error {
                    message: format!("invalid message: {err}"),
                    room: None,
                },
            );
            return;
        }
    };
    match parsed {
        ClientMessage::Subscribe { rooms } => {
            let mut accepted = Vec::with_capacity(rooms.len());
            for room in rooms {
                match resolve_and_authorize(&state.db, &room, user_id).await {
                    Ok(key) => {
                        state.hub.subscribe(conn_id, key);
                        accepted.push(room);
                    }
                    Err(msg) => send_ack(
                        &state.hub,
                        conn_id,
                        ServerAck::Error {
                            message: msg,
                            room: Some(room),
                        },
                    ),
                }
            }
            if !accepted.is_empty() {
                send_ack(
                    &state.hub,
                    conn_id,
                    ServerAck::Subscribed { rooms: accepted },
                );
            }
        }
        ClientMessage::Unsubscribe { rooms } => {
            let mut keys = Vec::with_capacity(rooms.len());
            for room in &rooms {
                if let Ok(key) = RoomKey::from_str(room) {
                    state.hub.unsubscribe(conn_id, key);
                    keys.push(room.clone());
                }
            }
            send_ack(&state.hub, conn_id, ServerAck::Unsubscribed { rooms: keys });
        }
        ClientMessage::Auth { .. } => { /* already authenticated */ }
    }
}

fn send_ack(hub: &Hub, conn_id: ConnId, ack: ServerAck) {
    if let Ok(payload) = serde_json::to_string(&ack) {
        hub.send_to(conn_id, payload);
    }
}

async fn resolve_and_authorize(
    pool: &PgPool,
    room: &str,
    user_id: Uuid,
) -> Result<RoomKey, String> {
    let key = RoomKey::from_str(room).map_err(|e| e.to_string())?;
    match key {
        RoomKey::Project(project_id) => {
            let team_id: Option<Uuid> = sqlx::query_scalar!(
                "SELECT team_id FROM projects WHERE id = $1 AND deleted_at IS NULL",
                project_id
            )
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("db error: {e}"))?;
            let team_id = team_id.ok_or_else(|| "project not found".to_string())?;
            require_team_role(pool, team_id, user_id, TeamRole::Viewer)
                .await
                .map_err(|_| "forbidden".to_string())?;
            Ok(key)
        }
        RoomKey::Team(team_id) => {
            require_team_role(pool, team_id, user_id, TeamRole::Viewer)
                .await
                .map_err(|_| "forbidden".to_string())?;
            Ok(key)
        }
        RoomKey::Channel(_) => {
            // Channels arrive in a later phase; reject for now.
            Err("channel rooms not yet supported".to_string())
        }
    }
}
