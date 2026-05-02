use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Subscription scope. Wire format: `"project:<uuid>"`, `"channel:<uuid>"`,
/// `"team:<uuid>"`. Used both as the registry key on the server and as the
/// `room` field in the wire envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomKey {
    Project(Uuid),
    Channel(Uuid),
    Team(Uuid),
}

impl fmt::Display for RoomKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoomKey::Project(id) => write!(f, "project:{id}"),
            RoomKey::Channel(id) => write!(f, "channel:{id}"),
            RoomKey::Team(id) => write!(f, "team:{id}"),
        }
    }
}

impl FromStr for RoomKey {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, id) = s
            .split_once(':')
            .ok_or_else(|| format!("invalid room key: {s}"))?;
        let id = Uuid::parse_str(id).map_err(|e| format!("invalid uuid in room: {e}"))?;
        match kind {
            "project" => Ok(RoomKey::Project(id)),
            "channel" => Ok(RoomKey::Channel(id)),
            "team" => Ok(RoomKey::Team(id)),
            _ => Err(format!("unknown room kind: {kind}")),
        }
    }
}

impl Serialize for RoomKey {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// Inbound message from a client. Tagged on the `type` field.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "auth")]
    Auth { token: String },
    #[serde(rename = "ws.subscribe")]
    Subscribe { rooms: Vec<String> },
    #[serde(rename = "ws.unsubscribe")]
    Unsubscribe { rooms: Vec<String> },
}

/// Outbound envelope sent to clients. Matches the wire protocol in CLAUDE.md.
#[derive(Debug, Clone, Serialize)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub payload: serde_json::Value,
    pub room: RoomKey,
    pub timestamp: DateTime<Utc>,
}

impl Envelope {
    pub fn new(event_type: &'static str, room: RoomKey, payload: serde_json::Value) -> Self {
        Self {
            event_type,
            payload,
            room,
            timestamp: Utc::now(),
        }
    }
}

/// Server-side ack/error responses to a client message. Sent inline (not
/// broadcast) to the originating connection.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ServerAck {
    #[serde(rename = "ws.subscribed")]
    Subscribed { rooms: Vec<String> },
    #[serde(rename = "ws.unsubscribed")]
    Unsubscribed { rooms: Vec<String> },
    #[serde(rename = "ws.error")]
    Error {
        message: String,
        room: Option<String>,
    },
}
