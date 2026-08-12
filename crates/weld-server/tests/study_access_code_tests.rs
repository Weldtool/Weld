//! Single-use access codes cap participation to one run per recruited person.
//! Proves the core invariant: a code provisions exactly once, a spent code is
//! rejected, and an unknown code is rejected — so the same person can't run the
//! study twice even on a fresh device where localStorage doesn't help.

use std::collections::HashMap;

use weld_server::errors::ApiError;
use weld_server::services::study::access_code::{self, CodeStatus};
use weld_server::services::study::provision::{
    provision_participant, resume_participant, update_progress,
};
use weld_server::services::study::survey::submit_interim_survey;

const JWT: &str = "test-jwt-secret-0123456789abcdef";

async fn pool() -> sqlx::PgPool {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    sqlx::PgPool::connect(&url)
        .await
        .expect("connect to test database")
}

#[tokio::test]
async fn unused_code_provisions_once_then_is_spent() {
    let pool = pool().await;
    let code = access_code::generate(&pool, 1, Some("test"))
        .await
        .expect("generate code")
        .pop()
        .expect("one code");

    // First use succeeds.
    provision_participant(&pool, JWT, Some("v1.0"), Some(&code), None, None)
        .await
        .expect("first provision with valid code");

    // Second use of the same code is rejected as already-used.
    match provision_participant(&pool, JWT, Some("v1.0"), Some(&code), None, None).await {
        Err(ApiError::Conflict(_)) => {}
        Ok(_) => panic!("spent code must not provision a second participant"),
        Err(e) => panic!("expected Conflict for a spent code, got {e:?}"),
    }
}

#[tokio::test]
async fn unknown_code_is_rejected() {
    let pool = pool().await;
    match provision_participant(
        &pool,
        JWT,
        Some("v1.0"),
        Some("WELD-ZZZZ-ZZZZ-nope"),
        None,
        None,
    )
    .await
    {
        Err(ApiError::Validation(_)) => {}
        Ok(_) => panic!("unknown code must not provision"),
        Err(e) => panic!("expected Validation for an unknown code, got {e:?}"),
    }
}

#[tokio::test]
async fn spent_code_resumes_existing_participant_at_saved_place() {
    let pool = pool().await;
    let code = access_code::generate(&pool, 1, Some("resume"))
        .await
        .expect("generate code")
        .pop()
        .expect("one code");

    let first = provision_participant(&pool, JWT, Some("v1.0"), Some(&code), None, None)
        .await
        .expect("first provision");

    // Participant advances partway through, saved server-side.
    update_progress(&pool, first.user_id, "tasks", 3)
        .await
        .expect("save progress");

    // Re-entering the (now spent) code resumes the same participant — no new
    // team/project — and hands back the saved place.
    let owner = match access_code::status(&pool, &code).await.expect("status") {
        CodeStatus::Used { user_id } => user_id,
        _ => panic!("expected Used status for a spent code"),
    };
    assert_eq!(
        owner, first.user_id,
        "code must point at the same participant"
    );

    let resumed = resume_participant(&pool, JWT, owner).await.expect("resume");
    assert!(resumed.resumed, "must be flagged as a resume");
    assert_eq!(resumed.team_id, first.team_id, "same isolated team");
    assert_eq!(resumed.project_id, first.project_id, "same project");
    assert_eq!(resumed.resume_stage.as_deref(), Some("tasks"));
    assert_eq!(resumed.resume_task_index, 3, "resumes at the saved task");
}

#[tokio::test]
async fn provision_without_code_still_works_for_testing() {
    // With STUDY_REQUIRE_ACCESS_CODE off, the route passes None and provisioning
    // proceeds — this path keeps local testing frictionless.
    let pool = pool().await;
    provision_participant(&pool, JWT, Some("v1.0"), None, None, None)
        .await
        .expect("provision without a code");
}

/// The post-baseline NASA-TLX is the only source of `tlx_*_baseline`, and it is
/// shown exactly once at the baseline→agent boundary. A participant who leaves
/// at that boundary and comes back on another device has no localStorage to
/// restore from, so the resume payload has to say the TLX is still owed —
/// otherwise the interstitial is skipped, those rows are never written, and H3's
/// delta is underivable for them with nothing in the data to show why.
#[tokio::test]
async fn resume_reports_whether_the_post_baseline_tlx_is_still_owed() {
    let pool = pool().await;
    let code = access_code::generate(&pool, 1, Some("tlx-resume"))
        .await
        .expect("generate code")
        .pop()
        .expect("one code");

    let p = provision_participant(&pool, JWT, Some("v1.2"), Some(&code), None, None)
        .await
        .expect("provision");

    // Parked on the first agent task (index 6) with the TLX unanswered.
    update_progress(&pool, p.user_id, "tasks", 6)
        .await
        .expect("save progress");

    let before = resume_participant(&pool, JWT, p.user_id)
        .await
        .expect("resume before TLX");
    assert!(
        !before.baseline_tlx_done,
        "TLX is unanswered, so the resume must still ask for it"
    );

    submit_interim_survey(
        &pool,
        p.user_id,
        HashMap::from([
            ("tlx_mental_baseline".to_string(), "55".to_string()),
            ("tlx_effort_baseline".to_string(), "40".to_string()),
        ]),
    )
    .await
    .expect("submit interim TLX");

    let after = resume_participant(&pool, JWT, p.user_id)
        .await
        .expect("resume after TLX");
    assert!(
        after.baseline_tlx_done,
        "once answered, a later resume must not re-administer it — re-rating the \
         manual phase after agent exposure would corrupt the baseline"
    );
}
