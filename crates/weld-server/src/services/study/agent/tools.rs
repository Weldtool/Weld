//! Agent tools and the double-gated Tool Executor (CLAUDE_AGENT_BUILD.md §3–§4).
//!
//! Tools are offered to the model only for the condition's allowed categories
//! (first gate, in the OpenAI request). The executor independently re-checks the
//! category before running anything (second gate) — so even if the model emits a
//! disallowed call it is rejected, never executed, and logged as an anomaly.
//! Every tool operates only on the participant's isolated team/project.

use std::sync::Arc;

use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
use weld_core::models::enums::Priority;

use super::ParticipantCtx;
use super::gate::{AgentCondition, ToolCategory, allowed_tool_categories};
use crate::errors::ApiError;
use crate::realtime::Hub;
use crate::realtime::broadcast::publish_project_event;
use crate::services::github::sync::link_task;
use crate::services::task::helpers::fetch_active_task;
use crate::services::task::{
    CreateTaskInput, TaskWithLabels, UpdateTaskInput, create_task, move_task, update_task,
};

const CREATE_CARD: &str = "create_card";
const MOVE_CARD: &str = "move_card";
const UPDATE_CARD: &str = "update_card";
const POST_MESSAGE: &str = "post_message";
const LINK_CARD_GITHUB: &str = "link_card_to_github";

/// Map a tool name to its capability category (None = unknown tool).
pub fn category_for(name: &str) -> Option<ToolCategory> {
    match name {
        CREATE_CARD | MOVE_CARD | UPDATE_CARD => Some(ToolCategory::BoardWrite),
        POST_MESSAGE => Some(ToolCategory::ChatPost),
        LINK_CARD_GITHUB => Some(ToolCategory::GithubLink),
        _ => None,
    }
}

/// OpenAI `tools` array for a condition — only functions whose category is
/// allowed are offered (first gate).
pub fn openai_tool_defs(condition: AgentCondition) -> Vec<Value> {
    let allowed = allowed_tool_categories(condition);
    all_defs()
        .into_iter()
        .filter(|(cat, _)| allowed.contains(cat))
        .map(|(_, def)| def)
        .collect()
}

fn all_defs() -> Vec<(ToolCategory, Value)> {
    vec![
        (
            ToolCategory::BoardWrite,
            function_def(
                CREATE_CARD,
                "Create a new board card (task).",
                json!({
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "column": { "type": "string", "description": "Target column name, e.g. Backlog / In Progress / Done" },
                        "description": { "type": "string" },
                        "priority": { "type": "string", "enum": ["critical", "high", "medium", "low", "none"] }
                    },
                    "required": ["title", "column"]
                }),
            ),
        ),
        (
            ToolCategory::BoardWrite,
            function_def(
                MOVE_CARD,
                "Move a card to a different column.",
                json!({
                    "type": "object",
                    "properties": {
                        "card_id": { "type": "string", "description": "The card's id from the board context" },
                        "to_column": { "type": "string", "description": "Destination column name" }
                    },
                    "required": ["card_id", "to_column"]
                }),
            ),
        ),
        (
            ToolCategory::BoardWrite,
            function_def(
                UPDATE_CARD,
                "Update a card's priority, assignee, or description.",
                json!({
                    "type": "object",
                    "properties": {
                        "card_id": { "type": "string" },
                        "priority": { "type": "string", "enum": ["critical", "high", "medium", "low", "none"] },
                        "assignee": { "type": "string", "description": "Teammate display name, or 'none' to unassign" },
                        "description": { "type": "string" }
                    },
                    "required": ["card_id"]
                }),
            ),
        ),
        (
            ToolCategory::ChatPost,
            function_def(
                POST_MESSAGE,
                "Draft a chat message for a team channel. This does NOT post it: the message is \
                 shown to the user to review, edit, and send themselves. Use it whenever the user \
                 wants something said in a channel.",
                json!({
                    "type": "object",
                    "properties": {
                        "channel": { "type": "string", "description": "Channel name, e.g. general or pettracker" },
                        "content": { "type": "string" }
                    },
                    "required": ["channel", "content"]
                }),
            ),
        ),
        (
            ToolCategory::GithubLink,
            function_def(
                LINK_CARD_GITHUB,
                "Link a board card to an existing GitHub issue or pull request.",
                json!({
                    "type": "object",
                    "properties": {
                        "card_id": { "type": "string" },
                        "issue_number": { "type": "integer" },
                        "pr_number": { "type": "integer" }
                    },
                    "required": ["card_id"]
                }),
            ),
        ),
    ]
}

fn function_def(name: &str, description: &str, parameters: Value) -> Value {
    json!({
        "type": "function",
        "function": { "name": name, "description": description, "parameters": parameters }
    })
}

/// Outcome of a successfully executed tool call.
pub struct ToolOutcome {
    /// Short result fed back to the model.
    pub result: String,
    /// Human-readable, attributable summary for the UI/telemetry
    /// ("Moved card X to In Progress").
    pub summary: String,
    /// The Weld entity this call touched, surfaced to the client so it can
    /// detect the participant later editing/undoing the agent's work
    /// (RQ1 verification/repair telemetry). `None` for actions that touch no
    /// trackable entity.
    pub entity: Option<EntityRef>,
    /// A chat message the agent proposes but does NOT post: the participant
    /// reviews/edits and sends it themselves (edit-before-send). `None` for any
    /// tool that isn't a chat draft.
    pub draft: Option<ChatDraft>,
}

/// A chat message the agent has drafted for the participant to review and send.
pub struct ChatDraft {
    /// Channel name without the leading '#'.
    pub channel: String,
    /// Resolved channel UUID (as a string) the client posts to on send.
    pub channel_id: String,
    /// The proposed message body, pre-filled into an editable box.
    pub content: String,
}

/// A reference to the entity an agent action created or modified. Threaded back
/// to the client (and logged on `agent_tool_call`) so user follow-up edits to
/// the same entity can be attributed as verification/repair behavior.
pub struct EntityRef {
    /// "card" or "message".
    pub kind: &'static str,
    /// The entity's UUID as a string.
    pub id: String,
    /// What the agent did: "created" | "moved" | "updated" | "linked" | "posted".
    pub action: &'static str,
}

/// Execute a tool call. Second gate: re-checks the category against the
/// condition and refuses if disallowed (returns `Forbidden`), so the run loop
/// can log `agent_tool_rejected`. Reuses the normal service layer, acting as the
/// participant (who has access to their own project).
pub async fn execute(
    pool: &PgPool,
    hub: &Arc<Hub>,
    ctx: &ParticipantCtx,
    name: &str,
    args: &Value,
) -> Result<ToolOutcome, ApiError> {
    // Second gate (defense in depth).
    let category =
        category_for(name).ok_or_else(|| ApiError::Forbidden(format!("Unknown tool: {name}")))?;
    if !allowed_tool_categories(ctx.condition).contains(&category) {
        return Err(ApiError::Forbidden(format!(
            "Tool {name} not permitted for condition {}",
            ctx.condition.as_str()
        )));
    }

    match name {
        CREATE_CARD => create_card(pool, hub, ctx, args).await,
        MOVE_CARD => move_card(pool, hub, ctx, args).await,
        UPDATE_CARD => update_card(pool, hub, ctx, args).await,
        POST_MESSAGE => post_chat(pool, hub, ctx, args).await,
        LINK_CARD_GITHUB => link_github(pool, hub, ctx, args).await,
        _ => Err(ApiError::Forbidden(format!("Unknown tool: {name}"))),
    }
}

// ── individual tools ─────────────────────────────────────────────────────────

async fn create_card(
    pool: &PgPool,
    hub: &Arc<Hub>,
    ctx: &ParticipantCtx,
    args: &Value,
) -> Result<ToolOutcome, ApiError> {
    let title = str_arg(args, "title")?;
    let column = str_arg(args, "column")?;
    let column_id = resolve_column(pool, ctx.project_id, &column).await?;
    let created = create_task(
        pool,
        ctx.project_id,
        ctx.user_id,
        CreateTaskInput {
            title: title.clone(),
            description: opt_str_arg(args, "description"),
            column_id,
            assignee_id: None,
            priority: parse_priority(args.get("priority").and_then(Value::as_str)),
            labels: None,
            due_date: None,
            estimate_hours: None,
        },
    )
    .await?;
    publish_task(hub, ctx.project_id, "task.created", &created);
    Ok(ToolOutcome {
        result: format!("Created card \"{title}\" in {column}."),
        summary: format!("Created card “{title}” in {column}"),
        entity: Some(EntityRef {
            kind: "card",
            id: created.task.id.to_string(),
            action: "created",
        }),
        draft: None,
    })
}

async fn move_card(
    pool: &PgPool,
    hub: &Arc<Hub>,
    ctx: &ParticipantCtx,
    args: &Value,
) -> Result<ToolOutcome, ApiError> {
    let card_id = uuid_arg(args, "card_id")?;
    let to_column = str_arg(args, "to_column")?;
    let column_id = resolve_column(pool, ctx.project_id, &to_column).await?;
    // Drop at the end of the destination column.
    let max_pos: Option<f64> = sqlx::query_scalar!(
        "SELECT MAX(position) FROM tasks WHERE column_id = $1 AND deleted_at IS NULL",
        column_id
    )
    .fetch_one(pool)
    .await?;
    let position = max_pos.unwrap_or(0.0) + 1.0;
    let moved = move_task(pool, card_id, ctx.user_id, column_id, position)
        .await?
        .task;
    publish_task(hub, ctx.project_id, "task.moved", &moved);
    Ok(ToolOutcome {
        result: format!("Moved card to {to_column}."),
        summary: format!("Moved “{}” to {to_column}", moved.task.title),
        entity: Some(EntityRef {
            kind: "card",
            id: card_id.to_string(),
            action: "moved",
        }),
        draft: None,
    })
}

async fn update_card(
    pool: &PgPool,
    hub: &Arc<Hub>,
    ctx: &ParticipantCtx,
    args: &Value,
) -> Result<ToolOutcome, ApiError> {
    let card_id = uuid_arg(args, "card_id")?;
    let assignee = match args.get("assignee").and_then(Value::as_str) {
        Some("none") => Some(None),
        Some(name) => Some(Some(resolve_assignee(pool, ctx.team_id, name).await?)),
        None => None,
    };
    let updated = update_task(
        pool,
        card_id,
        ctx.user_id,
        UpdateTaskInput {
            title: None,
            description: opt_str_arg(args, "description").map(Some),
            assignee_id: assignee,
            priority: parse_priority(args.get("priority").and_then(Value::as_str)),
            due_date: None,
            estimate_hours: None,
            labels: None,
        },
    )
    .await?;
    publish_task(hub, ctx.project_id, "task.updated", &updated);
    Ok(ToolOutcome {
        result: format!("Updated card “{}”.", updated.task.title),
        summary: format!("Updated card “{}”", updated.task.title),
        entity: Some(EntityRef {
            kind: "card",
            id: card_id.to_string(),
            action: "updated",
        }),
        draft: None,
    })
}

/// Draft (do NOT post) a chat message. Edit-before-send: the proposed message
/// is surfaced to the participant, who reviews/edits and sends it themselves
/// under their own name. This keeps chat attribution honest and produces the
/// accept/edit/reject signal for RQ1. We validate the channel exists so the
/// agent can't draft into a nonexistent channel, but we never write here.
async fn post_chat(
    pool: &PgPool,
    _hub: &Arc<Hub>,
    ctx: &ParticipantCtx,
    args: &Value,
) -> Result<ToolOutcome, ApiError> {
    let channel = str_arg(args, "channel")?;
    let content = str_arg(args, "content")?;
    let channel_id = resolve_channel(pool, ctx.team_id, &channel).await?;
    let clean = channel.trim().trim_start_matches('#').to_string();
    Ok(ToolOutcome {
        result: format!(
            "Drafted a message for #{clean}. It is shown to the user to review and send — it has \
             NOT been posted yet."
        ),
        summary: format!("Drafted a message for #{clean} (awaiting your review)"),
        entity: None,
        draft: Some(ChatDraft {
            channel: clean,
            channel_id: channel_id.to_string(),
            content,
        }),
    })
}

async fn link_github(
    pool: &PgPool,
    hub: &Arc<Hub>,
    ctx: &ParticipantCtx,
    args: &Value,
) -> Result<ToolOutcome, ApiError> {
    let card_id = uuid_arg(args, "card_id")?;
    let issue = args.get("issue_number").and_then(Value::as_i64);
    let pr = args.get("pr_number").and_then(Value::as_i64);
    if issue.is_none() && pr.is_none() {
        return Err(ApiError::Validation(
            "link_card_to_github needs an issue_number or pr_number".into(),
        ));
    }
    // Study GitHub is the seeded PetTracker repo; link writes only to Weld.
    let repo = "welddev/pettracker";
    // `link_task` writes both link fields, so passing only what the model
    // supplied would erase the card's other link — linking a PR would silently
    // drop a seeded issue link. Carry the existing values through, as the UI does.
    let existing = fetch_active_task(pool, card_id).await?;
    let linked = link_task(
        pool,
        card_id,
        ctx.user_id,
        repo,
        issue.or(existing.github_issue_id),
        pr.or(existing.github_pr_id),
    )
    .await?;
    publish_task(hub, ctx.project_id, "task.updated", &linked);
    let what = pr
        .map(|n| format!("PR #{n}"))
        .or_else(|| issue.map(|n| format!("issue #{n}")))
        .unwrap_or_default();
    Ok(ToolOutcome {
        result: format!("Linked card to {what}."),
        summary: format!("Linked “{}” to {what}", linked.task.title),
        entity: Some(EntityRef {
            kind: "card",
            id: card_id.to_string(),
            action: "linked",
        }),
        draft: None,
    })
}

// ── resolution + arg helpers ─────────────────────────────────────────────────

async fn resolve_column(pool: &PgPool, project_id: Uuid, name: &str) -> Result<Uuid, ApiError> {
    sqlx::query_scalar!(
        "SELECT id FROM columns WHERE project_id = $1 AND LOWER(name) = LOWER($2) LIMIT 1",
        project_id,
        name.trim()
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::Validation(format!("No column named '{name}'")))
}

async fn resolve_channel(pool: &PgPool, team_id: Uuid, name: &str) -> Result<Uuid, ApiError> {
    let cleaned = name.trim().trim_start_matches('#');
    sqlx::query_scalar!(
        "SELECT id FROM channels WHERE team_id = $1 AND LOWER(name) = LOWER($2) AND deleted_at IS NULL LIMIT 1",
        team_id,
        cleaned
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::Validation(format!("No channel named '{name}'")))
}

async fn resolve_assignee(pool: &PgPool, team_id: Uuid, name: &str) -> Result<Uuid, ApiError> {
    sqlx::query_scalar!(
        r#"
        SELECT u.id FROM users u
        JOIN team_memberships tm ON tm.user_id = u.id
        WHERE tm.team_id = $1 AND LOWER(u.display_name) = LOWER($2) LIMIT 1
        "#,
        team_id,
        name.trim()
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::Validation(format!("No teammate named '{name}'")))
}

fn parse_priority(s: Option<&str>) -> Option<Priority> {
    match s.map(str::to_lowercase).as_deref() {
        Some("critical") => Some(Priority::Critical),
        Some("high") => Some(Priority::High),
        Some("medium") => Some(Priority::Medium),
        Some("low") => Some(Priority::Low),
        Some("none") => Some(Priority::None),
        _ => None,
    }
}

fn str_arg(args: &Value, key: &str) -> Result<String, ApiError> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ApiError::Validation(format!("Missing argument: {key}")))
}

fn opt_str_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
}

fn uuid_arg(args: &Value, key: &str) -> Result<Uuid, ApiError> {
    let raw = str_arg(args, key)?;
    Uuid::parse_str(raw.trim())
        .map_err(|_| ApiError::Validation(format!("{key} is not a valid id")))
}

/// Publish a board task event using the same payload shape the REST routes emit
/// (the full task plus its label ids), so the client board upserts it live.
fn publish_task(hub: &Arc<Hub>, project_id: Uuid, event: &'static str, task: &TaskWithLabels) {
    let mut value = match serde_json::to_value(&task.task) {
        Ok(v) => v,
        Err(_) => return,
    };
    value["labels"] = json!(task.labels);
    publish_project_event(hub, project_id, event, &value);
}
