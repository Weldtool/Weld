use super::events::{Envelope, RoomKey};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use uuid::Uuid;

pub type ConnId = u64;

/// Sender half of a client's outbound channel; the WS task drains this and
/// writes to the socket. Cloning is cheap.
#[derive(Clone)]
pub struct ClientHandle {
    pub user_id: Uuid,
    pub sender: mpsc::UnboundedSender<String>,
}

#[derive(Default)]
struct HubInner {
    conns: HashMap<ConnId, ClientHandle>,
    rooms: HashMap<RoomKey, HashSet<ConnId>>,
    /// Reverse index: rooms each connection has joined, used so that
    /// `unregister` can clean up without scanning every room.
    conn_rooms: HashMap<ConnId, HashSet<RoomKey>>,
}

/// Single-process in-memory connection registry. Cloned across handlers via
/// `Arc<Hub>` placed in `AppState`. A Redis bridge can replace `broadcast`
/// later without touching call sites.
pub struct Hub {
    inner: Mutex<HubInner>,
    next_id: AtomicU64,
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Hub {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HubInner::default()),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn register(&self, handle: ClientHandle) -> ConnId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut inner = self.inner.lock().unwrap();
        inner.conns.insert(id, handle);
        id
    }

    pub fn unregister(&self, conn_id: ConnId) {
        let mut inner = self.inner.lock().unwrap();
        inner.conns.remove(&conn_id);
        if let Some(rooms) = inner.conn_rooms.remove(&conn_id) {
            for room in rooms {
                if let Some(set) = inner.rooms.get_mut(&room) {
                    set.remove(&conn_id);
                    if set.is_empty() {
                        inner.rooms.remove(&room);
                    }
                }
            }
        }
    }

    pub fn subscribe(&self, conn_id: ConnId, room: RoomKey) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.conns.contains_key(&conn_id) {
            return;
        }
        inner.rooms.entry(room).or_default().insert(conn_id);
        inner.conn_rooms.entry(conn_id).or_default().insert(room);
    }

    pub fn unsubscribe(&self, conn_id: ConnId, room: RoomKey) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(set) = inner.rooms.get_mut(&room) {
            set.remove(&conn_id);
            if set.is_empty() {
                inner.rooms.remove(&room);
            }
        }
        if let Some(set) = inner.conn_rooms.get_mut(&conn_id) {
            set.remove(&room);
        }
    }

    /// Serialize once, fan out to every connection currently in `room`. Send
    /// errors (closed receiver) are swallowed — the conn task will clean up
    /// via `unregister` shortly.
    pub fn broadcast(&self, envelope: &Envelope) {
        let payload = match serde_json::to_string(envelope) {
            Ok(s) => s,
            Err(err) => {
                tracing::error!(?err, "failed to serialize realtime envelope");
                return;
            }
        };
        let recipients: Vec<ClientHandle> = {
            let inner = self.inner.lock().unwrap();
            inner
                .rooms
                .get(&envelope.room)
                .map(|set| {
                    set.iter()
                        .filter_map(|id| inner.conns.get(id).cloned())
                        .collect()
                })
                .unwrap_or_default()
        };
        for handle in recipients {
            let _ = handle.sender.send(payload.clone());
        }
    }

    /// Send a serialized payload to a single connection. No-op if the
    /// connection is gone. Used for per-connection acks/errors.
    pub fn send_to(&self, conn_id: ConnId, payload: String) {
        let inner = self.inner.lock().unwrap();
        if let Some(handle) = inner.conns.get(&conn_id) {
            let _ = handle.sender.send(payload);
        }
    }

    #[cfg(test)]
    pub fn room_size(&self, room: &RoomKey) -> usize {
        self.inner
            .lock()
            .unwrap()
            .rooms
            .get(room)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub fn connection_count(&self) -> usize {
        self.inner.lock().unwrap().conns.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_handle() -> (ClientHandle, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            ClientHandle {
                user_id: Uuid::new_v4(),
                sender: tx,
            },
            rx,
        )
    }

    #[test]
    fn subscribe_and_broadcast_delivers_to_subscribers_only() {
        let hub = Hub::new();
        let project = RoomKey::Project(Uuid::new_v4());
        let other = RoomKey::Project(Uuid::new_v4());

        let (h1, mut rx1) = make_handle();
        let (h2, mut rx2) = make_handle();
        let c1 = hub.register(h1);
        let c2 = hub.register(h2);

        hub.subscribe(c1, project);
        hub.subscribe(c2, other);

        let env = Envelope::new("task.created", project, json!({"id":"x"}));
        hub.broadcast(&env);

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_err());
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let hub = Hub::new();
        let project = RoomKey::Project(Uuid::new_v4());
        let (h, mut rx) = make_handle();
        let c = hub.register(h);
        hub.subscribe(c, project);
        hub.unsubscribe(c, project);

        hub.broadcast(&Envelope::new("task.created", project, json!({})));
        assert!(rx.try_recv().is_err());
        assert_eq!(hub.room_size(&project), 0);
    }

    #[test]
    fn unregister_cleans_up_room_membership() {
        let hub = Hub::new();
        let project = RoomKey::Project(Uuid::new_v4());
        let (h, _rx) = make_handle();
        let c = hub.register(h);
        hub.subscribe(c, project);
        assert_eq!(hub.room_size(&project), 1);
        hub.unregister(c);
        assert_eq!(hub.room_size(&project), 0);
        assert_eq!(hub.connection_count(), 0);
    }

    #[test]
    fn fanout_serializes_once_to_multiple_subscribers() {
        let hub = Hub::new();
        let project = RoomKey::Project(Uuid::new_v4());
        let (h1, mut rx1) = make_handle();
        let (h2, mut rx2) = make_handle();
        let c1 = hub.register(h1);
        let c2 = hub.register(h2);
        hub.subscribe(c1, project);
        hub.subscribe(c2, project);

        hub.broadcast(&Envelope::new("task.created", project, json!({"a":1})));
        let m1 = rx1.try_recv().unwrap();
        let m2 = rx2.try_recv().unwrap();
        assert_eq!(m1, m2);
        assert!(m1.contains("\"task.created\""));
    }
}
