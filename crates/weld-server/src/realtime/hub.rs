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
    /// Connections per user — drives presence transitions.
    user_conns: HashMap<Uuid, HashSet<ConnId>>,
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

    /// Register a new connection. Returns `(conn_id, was_first_for_user)`
    /// where the bool is true iff this is the only live connection for
    /// `handle.user_id` — callers use that to drive `presence.online`.
    pub fn register(&self, handle: ClientHandle) -> (ConnId, bool) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let user_id = handle.user_id;
        let mut inner = self.inner.lock().unwrap();
        inner.conns.insert(id, handle);
        let entry = inner.user_conns.entry(user_id).or_default();
        let was_first = entry.is_empty();
        entry.insert(id);
        (id, was_first)
    }

    /// Tear down a connection. Returns `Some(user_id)` iff that user has no
    /// remaining live connections (caller can then schedule `presence.offline`).
    pub fn unregister(&self, conn_id: ConnId) -> Option<Uuid> {
        let mut inner = self.inner.lock().unwrap();
        let user_id = inner.conns.remove(&conn_id).map(|h| h.user_id);
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
        let user_id = user_id?;
        let now_empty = if let Some(set) = inner.user_conns.get_mut(&user_id) {
            set.remove(&conn_id);
            set.is_empty()
        } else {
            false
        };
        if now_empty {
            inner.user_conns.remove(&user_id);
            Some(user_id)
        } else {
            None
        }
    }

    /// Is the user currently connected (>=1 live socket)?
    pub fn user_is_online(&self, user_id: Uuid) -> bool {
        let inner = self.inner.lock().unwrap();
        inner
            .user_conns
            .get(&user_id)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
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

    /// Every room any of `user_id`'s live connections is subscribed to.
    pub fn rooms_for_user(&self, user_id: Uuid) -> Vec<RoomKey> {
        let inner = self.inner.lock().unwrap();
        let mut rooms: HashSet<RoomKey> = HashSet::new();
        if let Some(conns) = inner.user_conns.get(&user_id) {
            for conn in conns {
                if let Some(joined) = inner.conn_rooms.get(conn) {
                    rooms.extend(joined.iter().copied());
                }
            }
        }
        rooms.into_iter().collect()
    }

    /// Remove all of `user_id`'s connections from `room` — used when their
    /// access to it is revoked while they are still connected.
    pub fn unsubscribe_user(&self, user_id: Uuid, room: RoomKey) {
        let mut inner = self.inner.lock().unwrap();
        let conns: Vec<ConnId> = inner
            .user_conns
            .get(&user_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        for conn in conns {
            if let Some(set) = inner.rooms.get_mut(&room) {
                set.remove(&conn);
            }
            if let Some(set) = inner.conn_rooms.get_mut(&conn) {
                set.remove(&room);
            }
        }
        if inner.rooms.get(&room).is_some_and(|set| set.is_empty()) {
            inner.rooms.remove(&room);
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
        let (c1, _) = hub.register(h1);
        let (c2, _) = hub.register(h2);

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
        let (c, _) = hub.register(h);
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
        let (c, _) = hub.register(h);
        hub.subscribe(c, project);
        assert_eq!(hub.room_size(&project), 1);
        hub.unregister(c);
        assert_eq!(hub.room_size(&project), 0);
        assert_eq!(hub.connection_count(), 0);
    }

    #[test]
    fn unsubscribe_user_drops_every_connection_of_that_user_only() {
        let hub = Hub::new();
        let room = RoomKey::Channel(Uuid::new_v4());
        let other_room = RoomKey::Channel(Uuid::new_v4());

        let (first, mut rx_first) = make_handle();
        let user_id = first.user_id;
        let (tx_second, mut rx_second) = mpsc::unbounded_channel();
        let second = ClientHandle {
            user_id,
            sender: tx_second,
        };
        let (bystander, mut rx_bystander) = make_handle();

        let (c1, _) = hub.register(first);
        let (c2, _) = hub.register(second);
        let (c3, _) = hub.register(bystander);
        hub.subscribe(c1, room);
        hub.subscribe(c2, room);
        hub.subscribe(c2, other_room);
        hub.subscribe(c3, room);

        let mut rooms = hub.rooms_for_user(user_id);
        rooms.sort_by_key(|r| r.to_string());
        let mut expected = vec![room, other_room];
        expected.sort_by_key(|r| r.to_string());
        assert_eq!(rooms, expected);

        hub.unsubscribe_user(user_id, room);
        hub.broadcast(&Envelope::new("chat.message.new", room, json!({})));

        assert!(rx_first.try_recv().is_err());
        assert!(rx_second.try_recv().is_err());
        assert!(rx_bystander.try_recv().is_ok());
        assert_eq!(hub.rooms_for_user(user_id), vec![other_room]);
    }

    #[test]
    fn fanout_serializes_once_to_multiple_subscribers() {
        let hub = Hub::new();
        let project = RoomKey::Project(Uuid::new_v4());
        let (h1, mut rx1) = make_handle();
        let (h2, mut rx2) = make_handle();
        let (c1, _) = hub.register(h1);
        let (c2, _) = hub.register(h2);
        hub.subscribe(c1, project);
        hub.subscribe(c2, project);

        hub.broadcast(&Envelope::new("task.created", project, json!({"a":1})));
        let m1 = rx1.try_recv().unwrap();
        let m2 = rx2.try_recv().unwrap();
        assert_eq!(m1, m2);
        assert!(m1.contains("\"task.created\""));
    }
}
