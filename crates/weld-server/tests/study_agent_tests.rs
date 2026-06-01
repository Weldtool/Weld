//! Proof-of-scoping for the study agent's Context Gate (CLAUDE_AGENT_BUILD.md
//! §9). Provisions a real participant, then assembles context for each condition
//! against real seeded data and asserts the hard boundary holds: C1 never sees
//! board or GitHub, C2 never sees GitHub.

use weld_server::services::study::agent::gate::{self, AgentCondition};
use weld_server::services::study::agent::{ParticipantCtx, load_participant};
use weld_server::services::study::provision::provision_participant;

async fn pool() -> sqlx::PgPool {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    sqlx::PgPool::connect(&url)
        .await
        .expect("connect to test database")
}

/// Provision a participant and return a ParticipantCtx loaded from the DB.
async fn provision_ctx(pool: &sqlx::PgPool) -> ParticipantCtx {
    let res = provision_participant(pool, "test-jwt-secret-0123456789abcdef", Some("v1.0"), None)
        .await
        .expect("provision participant");
    load_participant(pool, res.user_id)
        .await
        .expect("load participant")
        .expect("participant exists")
}

#[tokio::test]
async fn c1_context_contains_chat_only() {
    let pool = pool().await;
    let mut ctx = provision_ctx(&pool).await;
    ctx.condition = AgentCondition::C1;

    let scoped = gate::assemble(&pool, &ctx).await.expect("assemble C1");
    let snap = scoped.snapshot();

    assert!(!scoped.chat.is_empty(), "C1 must receive chat");
    assert!(scoped.board.is_none(), "C1 must NOT assemble board");
    assert!(scoped.github.is_none(), "C1 must NOT assemble github");
    assert!(
        snap.get("board").is_none(),
        "C1 snapshot must not contain board"
    );
    assert!(
        snap.get("github").is_none(),
        "C1 snapshot must not contain github"
    );
    assert_eq!(snap["sources_included"], serde_json::json!(["chat"]));
}

#[tokio::test]
async fn c2_context_has_board_but_not_github() {
    let pool = pool().await;
    let mut ctx = provision_ctx(&pool).await;
    ctx.condition = AgentCondition::C2;

    let scoped = gate::assemble(&pool, &ctx).await.expect("assemble C2");
    let snap = scoped.snapshot();

    assert!(scoped.board.is_some(), "C2 must assemble board");
    assert!(scoped.github.is_none(), "C2 must NOT assemble github");
    assert!(
        snap.get("github").is_none(),
        "C2 snapshot must not contain github"
    );
}

#[tokio::test]
async fn c3_context_has_everything() {
    let pool = pool().await;
    let mut ctx = provision_ctx(&pool).await;
    ctx.condition = AgentCondition::C3;

    let scoped = gate::assemble(&pool, &ctx).await.expect("assemble C3");

    assert!(!scoped.chat.is_empty(), "C3 must receive chat");
    assert!(scoped.board.is_some(), "C3 must assemble board");
    let github = scoped.github.expect("C3 must assemble github");
    assert!(!github.is_empty(), "C3 github fixtures should be non-empty");
}
