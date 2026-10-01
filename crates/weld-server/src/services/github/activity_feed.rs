use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::realtime::Hub;
use crate::realtime::broadcast::publish_channel_event;
use crate::routes::chat::MessageResponse;

/// Fixed UUID for the synthetic github-bot user (seeded by migration).
pub const GITHUB_BOT_ID: Uuid = Uuid::from_u128(2);

/// Post a PR event as a system message to all channels subscribed to `repo`
/// that have the "pr" filter enabled.
#[tracing::instrument(skip(db, hub, payload))]
pub async fn post_pr_event(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    payload: &serde_json::Value,
    action: &str,
    merged: bool,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    let kind = match (action, merged) {
        ("opened", _) => "pr.opened",
        ("closed", true) => "pr.merged",
        ("closed", false) => "pr.closed",
        ("reopened", _) => "pr.reopened",
        // A draft becoming ready is when the team is actually being asked to
        // look at it — previously this event was dropped on the floor.
        ("ready_for_review", _) => "pr.ready",
        ("review_requested", _) => "pr.review_requested",
        _ => return Ok(()),
    };

    let pr = &payload["pull_request"];
    let number = pr["number"].as_i64().unwrap_or(0);
    let title = pr["title"].as_str().unwrap_or("").to_string();
    let url = pr["html_url"].as_str().unwrap_or("").to_string();
    let author = pr["user"]["login"].as_str().unwrap_or("").to_string();
    let author_avatar = pr["user"]["avatar_url"].as_str().unwrap_or("").to_string();
    let additions = pr["additions"].as_i64().unwrap_or(0);
    let deletions = pr["deletions"].as_i64().unwrap_or(0);

    let verb = match kind {
        "pr.opened" => "opened",
        "pr.merged" => "merged",
        "pr.closed" => "closed",
        "pr.ready" => "marked ready for review",
        "pr.review_requested" => "requested review on",
        _ => "reopened",
    };

    // "requested review on #12" reads backwards with the author-last phrasing
    // the other verbs use, so that one kind gets its own sentence.
    let content = if kind == "pr.review_requested" {
        let reviewer = payload["requested_reviewer"]["login"]
            .as_str()
            .unwrap_or("someone");
        format!("**@{author}** requested review from **@{reviewer}** on [#{number} {title}]({url})")
    } else {
        format!("**[#{number} {title}]({url})** {verb} by @{author} (+{additions} -{deletions})")
    };
    let metadata = json!({
        "kind": kind,
        "repo": repo,
        "number": number,
        "title": title,
        "url": url,
        "author": author,
        "author_avatar": author_avatar,
        "additions": additions,
        "deletions": deletions,
    });

    post_to_subscribed_channels(db, hub, repo, "pr", content, metadata, teams).await
}

/// Post an issue event as a system message to subscribed channels.
#[tracing::instrument(skip(db, hub, payload))]
pub async fn post_issue_event(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    payload: &serde_json::Value,
    action: &str,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    let kind = match action {
        "opened" => "issue.opened",
        "closed" => "issue.closed",
        "reopened" => "issue.reopened",
        _ => return Ok(()),
    };

    let issue = &payload["issue"];
    let number = issue["number"].as_i64().unwrap_or(0);
    let title = issue["title"].as_str().unwrap_or("").to_string();
    let url = issue["html_url"].as_str().unwrap_or("").to_string();
    let author = issue["user"]["login"].as_str().unwrap_or("").to_string();
    let author_avatar = issue["user"]["avatar_url"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let labels: Vec<String> = issue["labels"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let verb = match kind {
        "issue.opened" => "opened",
        "issue.closed" => "closed",
        _ => "reopened",
    };
    let content = format!("**[#{number} {title}]({url})** {verb} by @{author}");
    let metadata = json!({
        "kind": kind,
        "repo": repo,
        "number": number,
        "title": title,
        "url": url,
        "author": author,
        "author_avatar": author_avatar,
        "labels": labels,
    });

    post_to_subscribed_channels(db, hub, repo, "issue", content, metadata, teams).await
}

/// Post a PR review event as a system message to subscribed channels.
#[tracing::instrument(skip(db, hub, payload))]
pub async fn post_review_event(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    payload: &serde_json::Value,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    let review = &payload["review"];
    let state = review["state"].as_str().unwrap_or("");
    let verdict = match state {
        "approved" => "approved",
        "changes_requested" => "changes_requested",
        "commented" => "commented",
        _ => return Ok(()),
    };

    let pr = &payload["pull_request"];
    let number = pr["number"].as_i64().unwrap_or(0);
    let url = review["html_url"].as_str().unwrap_or("").to_string();
    let author = review["user"]["login"].as_str().unwrap_or("").to_string();
    let author_avatar = review["user"]["avatar_url"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let verdict_label = match verdict {
        "approved" => "approved",
        "changes_requested" => "requested changes on",
        _ => "commented on",
    };
    let content = format!("**@{author}** {verdict_label} [#{number}]({url})");
    let metadata = json!({
        "kind": "review.submitted",
        "repo": repo,
        "number": number,
        "url": url,
        "author": author,
        "author_avatar": author_avatar,
        "verdict": verdict,
    });

    post_to_subscribed_channels(db, hub, repo, "review", content, metadata, teams).await
}

async fn post_to_subscribed_channels(
    db: &PgPool,
    hub: &Arc<Hub>,
    repo: &str,
    filter_key: &str,
    content: String,
    metadata: serde_json::Value,
    teams: &[Uuid],
) -> Result<(), ApiError> {
    // Find channels subscribed to this repo with the event type enabled.
    // Scoped to the teams that own the installation: a channel subscribing to
    // the string "acme/api" must not receive events from someone else's repo
    // that happens to share the name.
    let channels = sqlx::query!(
        r#"
        SELECT id
        FROM channels
        WHERE $1 = ANY(github_repos)
          AND deleted_at IS NULL
          AND team_id = ANY($3)
          AND (github_event_filter->>$2)::boolean IS TRUE
        "#,
        repo,
        filter_key,
        teams,
    )
    .fetch_all(db)
    .await?;

    for row in channels {
        let msg = sqlx::query!(
            r#"
            INSERT INTO messages (channel_id, sender_id, content, metadata, mentions)
            VALUES ($1, $2, $3, $4, '{}')
            RETURNING id, channel_id, sender_id, content, thread_parent_id,
                      mentions as "mentions!: Vec<Uuid>",
               task_refs as "task_refs!: Vec<Uuid>",
                      metadata, edited_at, created_at
            "#,
            row.id,
            GITHUB_BOT_ID,
            content,
            metadata,
        )
        .fetch_one(db)
        .await?;

        let resp = MessageResponse {
            id: msg.id,
            channel_id: msg.channel_id,
            sender_id: msg.sender_id,
            content: msg.content,
            thread_parent_id: msg.thread_parent_id,
            mentions: msg.mentions,
            task_refs: msg.task_refs,
            metadata: msg.metadata,
            edited_at: msg.edited_at,
            created_at: msg.created_at,
            reply_count: None,
        };

        publish_channel_event(hub, row.id, "chat.message.new", &resp);
        tracing::info!(channel_id = %row.id, repo, "Posted GitHub activity feed message");
    }

    Ok(())
}
