use crate::middleware::auth::Claims;
use crate::realtime::Hub;
use crate::realtime::broadcast::{publish_channel_event, publish_team_event};
use crate::realtime::events::{ClientMessage, Envelope, RoomKey, ServerAck};
use crate::realtime::hub::{ClientHandle, ConnId};
use crate::routes::chat::MessageResponse;
use crate::services::chat::helpers::{fetch_active_channel, require_channel_access};
use crate::services::project::helpers::{fetch_active_project, require_project_team_role};
use crate::services::team::helpers::require_team_role;
use crate::state::AppState;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;
use weld_core::models::enums::TeamRole;
use weld_core::models::message::ChatMessage;

/// Grace period before broadcasting `presence.offline` after a user's last
/// connection drops. Bridges brief network blips and page reloads.
const PRESENCE_OFFLINE_GRACE: Duration = Duration::from_secs(5);

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
    let (conn_id, was_first) = state.hub.register(ClientHandle {
        user_id,
        sender: tx,
    });

    if was_first {
        broadcast_presence(&state, user_id, "presence.online").await;
    }

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

    let became_empty = state.hub.unregister(conn_id);
    writer.abort();

    if became_empty.is_some() {
        let state_clone = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(PRESENCE_OFFLINE_GRACE).await;
            // If the user reconnected during the grace window, suppress the
            // offline event — they're still online.
            if !state_clone.hub.user_is_online(user_id) {
                broadcast_presence(&state_clone, user_id, "presence.offline").await;
            }
        });
    }
}

/// Fetch every team the user belongs to and broadcast a presence event to
/// each `team:<uuid>` room. Used for both online and offline transitions.
async fn broadcast_presence(state: &AppState, user_id: Uuid, event_type: &'static str) {
    let teams: Vec<Uuid> = match sqlx::query_scalar!(
        "SELECT team_id FROM team_memberships WHERE user_id = $1",
        user_id
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::warn!(?err, %user_id, "presence team lookup failed");
            return;
        }
    };
    let payload = serde_json::json!({ "user_id": user_id });
    for team_id in teams {
        publish_team_event(&state.hub, team_id, event_type, &payload);
    }
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
        ClientMessage::Subscribe { rooms, since } => {
            let mut accepted = Vec::with_capacity(rooms.len());
            let mut accepted_keys = Vec::with_capacity(rooms.len());
            for room in rooms {
                match resolve_and_authorize(&state.db, &room, user_id).await {
                    Ok(key) => {
                        state.hub.subscribe(conn_id, key);
                        accepted.push(room);
                        accepted_keys.push(key);
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
            // After ack: replay any messages newer than `since` to this
            // connection only. Skipped for non-channel rooms.
            if let Some(since) = since {
                for key in accepted_keys {
                    if let RoomKey::Channel(channel_id) = key {
                        backfill_channel(&state.db, &state.hub, conn_id, channel_id, since).await;
                    }
                }
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
        ClientMessage::ChatTyping { channel_id } => {
            // Re-broadcast as chat.user.typing only if the user can access the
            // channel. Don't echo back to the typing user.
            let channel = match fetch_active_channel(&state.db, channel_id).await {
                Ok(c) => c,
                Err(_) => return,
            };
            if require_channel_access(&state.db, &channel, user_id)
                .await
                .is_err()
            {
                return;
            }
            publish_channel_event(
                &state.hub,
                channel_id,
                "chat.user.typing",
                &serde_json::json!({
                    "channel_id": channel_id,
                    "user_id": user_id,
                }),
            );
        }
        ClientMessage::Auth { .. } => { /* already authenticated */ }
    }
}

fn send_ack(hub: &Hub, conn_id: ConnId, ack: ServerAck) {
    if let Ok(payload) = serde_json::to_string(&ack) {
        hub.send_to(conn_id, payload);
    }
}

/// Replay messages in `channel_id` newer than `since` directly to `conn_id`
/// as `chat.message.new` envelopes. Capped to avoid pathological backfills.
async fn backfill_channel(
    pool: &PgPool,
    hub: &Arc<Hub>,
    conn_id: ConnId,
    channel_id: Uuid,
    since: DateTime<Utc>,
) {
    const BACKFILL_MAX: i64 = 200;
    let rows = match sqlx::query_as!(
        ChatMessage,
        r#"
        SELECT id, channel_id, sender_id, content, thread_parent_id,
               mentions as "mentions!: Vec<Uuid>",
               task_refs as "task_refs!: Vec<Uuid>",
               metadata, edited_at, deleted_at, created_at
        FROM messages
        WHERE channel_id = $1
          AND deleted_at IS NULL
          AND created_at > $2
        ORDER BY created_at ASC
        LIMIT $3
        "#,
        channel_id,
        since,
        BACKFILL_MAX,
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(err) => {
            tracing::warn!(?err, %channel_id, "backfill query failed");
            return;
        }
    };
    for msg in rows {
        let resp: MessageResponse = msg.into();
        let payload = match serde_json::to_value(&resp) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let envelope = Envelope::new("chat.message.new", RoomKey::Channel(channel_id), payload);
        if let Ok(text) = serde_json::to_string(&envelope) {
            hub.send_to(conn_id, text);
        }
    }
}

async fn resolve_and_authorize(
    pool: &PgPool,
    room: &str,
    user_id: Uuid,
) -> Result<RoomKey, String> {
    let key = RoomKey::from_str(room).map_err(|e| e.to_string())?;
    authorize_room(pool, key, user_id).await
}

/// Checks that `user_id` may currently listen to `key`.
async fn authorize_room(pool: &PgPool, key: RoomKey, user_id: Uuid) -> Result<RoomKey, String> {
    match key {
        RoomKey::Project(project_id) => {
            let project = fetch_active_project(pool, project_id)
                .await
                .map_err(|_| "project not found".to_string())?;
            // Project-level access: being on the team is not enough for
            // Members/Viewers, so a board they can't open over REST must not
            // stream to them over the socket either.
            require_project_team_role(pool, &project, user_id, TeamRole::Viewer)
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
        RoomKey::Channel(channel_id) => {
            let channel = fetch_active_channel(pool, channel_id)
                .await
                .map_err(|_| "channel not found".to_string())?;
            require_channel_access(pool, &channel, user_id)
                .await
                .map_err(|_| "forbidden".to_string())?;
            Ok(key)
        }
    }
}

/// Drops any of `user_id`'s live subscriptions they are no longer entitled to.
///
/// Authorisation is otherwise only checked when a room is joined, so someone
/// removed from a private channel, a project or a whole team would keep
/// receiving its events until they happened to reconnect. Call this after any
/// change that can take access away.
pub async fn revalidate_user_rooms(state: &AppState, user_id: Uuid) {
    for room in state.hub.rooms_for_user(user_id) {
        if authorize_room(&state.db, room, user_id).await.is_err() {
            state.hub.unsubscribe_user(user_id, room);
        }
    }
}
