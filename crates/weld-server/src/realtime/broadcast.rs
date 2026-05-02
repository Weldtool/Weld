use super::events::{Envelope, RoomKey};
use super::hub::Hub;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

/// Serialize `payload` and broadcast it to the project room. Failures are
/// logged and dropped — a board mutation must not be rolled back because the
/// realtime layer hiccupped.
pub fn publish_project_event<T: Serialize>(
    hub: &Arc<Hub>,
    project_id: Uuid,
    event_type: &'static str,
    payload: &T,
) {
    let value = match serde_json::to_value(payload) {
        Ok(v) => v,
        Err(err) => {
            tracing::error!(?err, event_type, "failed to serialize realtime payload");
            return;
        }
    };
    let envelope = Envelope::new(event_type, RoomKey::Project(project_id), value);
    hub.broadcast(&envelope);
}
