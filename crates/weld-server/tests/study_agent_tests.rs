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
    let res = provision_participant(
        pool,
        "test-jwt-secret-0123456789abcdef",
        Some("v1.0"),
        None,
        None,
        None,
    )
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

    let board = scoped.board.as_ref().expect("C2 must assemble board");
    assert!(scoped.github.is_none(), "C2 must NOT assemble github");
    assert!(
        snap.get("github").is_none(),
        "C2 snapshot must not contain github"
    );
    // The board carries no card→issue/PR link numbers either: those are GitHub
    // references and must be absent for a no-GitHub condition, even though the
    // seed pre-links some cards (e.g. card_feeding_schedule_ui → PR #22).
    assert!(
        board
            .iter()
            .all(|c| c.github_issue.is_none() && c.github_pr.is_none()),
        "C2 board cards must expose no GitHub link numbers"
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

/// Linking a PR through the agent's tool must not wipe the card's issue link
/// (and vice versa): `link_task` writes both fields, so the tool has to carry
/// the one the model didn't mention.
#[tokio::test]
async fn agent_link_tool_preserves_the_cards_other_link() {
    use serde_json::json;
    use weld_server::services::github::sync::link_task;
    use weld_server::services::study::agent::tools;

    let pool = pool().await;
    let mut ctx = provision_ctx(&pool).await;
    ctx.condition = AgentCondition::C3;
    let hub = std::sync::Arc::new(weld_server::realtime::Hub::new());

    let card_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM tasks WHERE project_id = $1 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(ctx.project_id)
    .fetch_one(&pool)
    .await
    .expect("a seeded card");

    link_task(
        &pool,
        card_id,
        ctx.user_id,
        "welddev/pettracker",
        Some(7),
        None,
    )
    .await
    .expect("link issue");

    tools::execute(
        &pool,
        &hub,
        &ctx,
        "link_card_to_github",
        &json!({"card_id": card_id.to_string(), "pr_number": 22}),
    )
    .await
    .expect("agent links the PR");

    let (issue, pr): (Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT github_issue_id, github_pr_id FROM tasks WHERE id = $1")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pr, Some(22));
    assert_eq!(issue, Some(7), "the existing issue link was erased");
}
