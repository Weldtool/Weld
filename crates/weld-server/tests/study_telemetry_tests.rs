//! The telemetry batch endpoint is the only record of participant behaviour, so
//! one malformed event must never cost a session. The client re-queues a failed
//! batch, which means rejecting a whole batch for one unknown type would poison
//! the buffer and silently drop every later event. These tests pin the
//! partition behaviour: known types are stored, unknown types are dropped, and
//! a mixed batch still succeeds.

use weld_server::services::study::provision::provision_participant;
use weld_server::services::study::telemetry::{self, EventInput};

const JWT: &str = "test-jwt-secret-0123456789abcdef";

async fn pool() -> sqlx::PgPool {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    sqlx::PgPool::connect(&url)
        .await
        .expect("connect to test database")
}

fn event(event_type: &str) -> EventInput {
    EventInput {
        event_type: event_type.to_string(),
        current_task_id: Some("T6".into()),
        payload: None,
        client_ts: None,
    }
}

#[tokio::test]
async fn mixed_batch_stores_known_events_and_drops_unknown() {
    let pool = pool().await;
    let p = provision_participant(&pool, JWT, Some("v1.0"), None, None, None)
        .await
        .expect("provision participant");

    let stored = telemetry::record_events(
        &pool,
        p.user_id,
        vec![
            event("task_shown"),
            event("definitely_not_a_real_event"),
            event("task_completed"),
        ],
    )
    .await
    .expect("a batch with one bad event must still succeed");

    assert_eq!(stored, 2, "both known events stored, unknown one dropped");

    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM study_events e
         JOIN study_participants sp ON sp.id = e.participant_id
         WHERE sp.user_id = $1 AND e.event_type = 'definitely_not_a_real_event'",
    )
    .bind(p.user_id)
    .fetch_one(&pool)
    .await
    .expect("count unknown events");
    assert_eq!(n, 0, "unknown event types must never reach the database");
}

#[tokio::test]
async fn pr_review_events_are_accepted() {
    let pool = pool().await;
    let p = provision_participant(&pool, JWT, Some("v1.0"), None, None, None)
        .await
        .expect("provision participant");

    // Instrumented by PrReviewOverlay. These shipped in the client before they
    // existed here, so every B4/T6 review silently 400'd its whole batch.
    let stored = telemetry::record_events(
        &pool,
        p.user_id,
        vec![event("pr_review_opened"), event("pr_review_submitted")],
    )
    .await
    .expect("record client-instrumented events");

    assert_eq!(stored, 2, "both PR-review event types must be allowed");
}
