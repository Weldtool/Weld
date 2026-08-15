//! The export endpoint is the only way analysis data leaves the server, so a
//! table the server writes but never exports is invisible until run day — at
//! which point the study is over and the data is stranded.
//!
//! That is exactly what happened to `study_agent_turns`: the agent wrote every
//! turn, the analysis plan named it as the source for correctness coding and
//! H4's agent-error coding, and no dataset served it. These tests pin the
//! contract the analysis pipeline reads against: every dataset name parses, and
//! `agent_turns` produces the header `load.py` expects with the turns in order.

use weld_server::services::study::export::{Dataset, export_csv};
use weld_server::services::study::provision::provision_participant;

const JWT: &str = "test-jwt-secret-0123456789abcdef";

async fn pool() -> sqlx::PgPool {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    sqlx::PgPool::connect(&url)
        .await
        .expect("connect to test database")
}

#[tokio::test]
async fn every_documented_dataset_name_parses() {
    for name in [
        "participants",
        "events",
        "survey",
        "agent_turns",
        "access_codes",
    ] {
        assert!(
            Dataset::parse(name).is_ok(),
            "`{name}` is documented in the export module and the analysis \
             Makefile fetches it; it must parse"
        );
    }
    assert!(Dataset::parse("not_a_dataset").is_err());
}

#[tokio::test]
async fn agent_turns_export_matches_the_analysis_pipeline_contract() {
    let pool = pool().await;
    let p = provision_participant(&pool, JWT, Some("v1.0"), None, None, None)
        .await
        .expect("provision participant");

    // Two turns on one task, inserted out of chronological order on purpose:
    // the export must order them, because a coder reading a transcript needs the
    // exchange in sequence and `turn_index` is what the packet builder sorts on.
    let participant_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM study_participants WHERE participant_code = $1")
            .bind(&p.participant_code)
            .fetch_one(&pool)
            .await
            .expect("look up participant row");

    for (role, content, offset) in [
        ("assistant", "Here is what I found.", 5_i32),
        ("user", "Which PR closes this card?", 0),
    ] {
        sqlx::query(
            "INSERT INTO study_agent_turns (participant_id, task_id, role, content, created_at)
             VALUES ($1, 'T6', $2, $3, NOW() + ($4 || ' seconds')::interval)",
        )
        .bind(participant_id)
        .bind(role)
        .bind(content)
        .bind(offset)
        .execute(&pool)
        .await
        .expect("insert agent turn");
    }

    let csv = export_csv(&pool, Dataset::AgentTurns)
        .await
        .expect("export agent turns");
    let mut lines = csv.lines();

    // load.py asserts this column set. If it drifts, the pipeline raises there
    // rather than silently analysing a column that no longer means what it did.
    assert_eq!(
        lines.next().expect("header row"),
        "participant_code,agent_condition,task_id,turn_index,role,content,created_at"
    );

    let ours: Vec<&str> = lines
        .filter(|l| l.starts_with(&p.participant_code))
        .collect();
    assert_eq!(ours.len(), 2, "both turns should export");
    assert!(
        ours[0].contains(",1,user,"),
        "the earlier turn must be index 1 and come first, got: {}",
        ours[0]
    );
    assert!(
        ours[1].contains(",2,assistant,"),
        "the later turn must be index 2, got: {}",
        ours[1]
    );
}
