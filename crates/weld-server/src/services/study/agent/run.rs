//! Orchestrates one agent turn (CLAUDE_AGENT_BUILD.md §2 flow): assemble scoped
//! context at the Gate → call the pinned model with the condition's allowed
//! tools → execute tool calls through the double-gated executor → record the
//! study-critical telemetry (`agent_invoked`, `agent_context_sent`,
//! `agent_response`, `agent_tool_call`, `agent_tool_rejected`).

use std::sync::Arc;

use serde_json::{Value, json};
use sqlx::PgPool;

use super::gate::{self, AgentCondition};
use super::{ParticipantCtx, openai, tools};
use crate::errors::ApiError;
use crate::realtime::Hub;
use crate::services::study::telemetry::record_server_event;
use crate::state::AgentConfig;

/// Max model⇄tool round-trips before we stop (guards against loops).
const MAX_TURNS: usize = 5;

/// Hard cap on agent messages per participant for the whole session. A normal
/// run is well under this; the cap exists only to stop a spammer from running
/// up unbounded API cost. Counted from `agent_invoked` events.
const MAX_AGENT_MESSAGES: i64 = 50;

pub struct AgentRunResult {
    /// The agent's final text reply to show the user.
    pub reply: String,
    /// Attributable summaries of actions the agent performed (for the UI).
    pub actions: Vec<String>,
    /// The entities the agent created/modified this turn, so the client can
    /// attribute later user edits to them as verification/repair (RQ1).
    pub entities: Vec<AgentEntity>,
    /// Chat messages the agent drafted for the participant to review and send
    /// (edit-before-send). Not yet posted.
    pub drafts: Vec<AgentDraft>,
    pub condition: String,
}

/// A trackable entity an agent action touched, surfaced to the client.
#[derive(serde::Serialize)]
pub struct AgentEntity {
    pub kind: String,
    pub id: String,
    pub action: String,
}

/// A chat message the agent proposed; the participant edits/sends it.
#[derive(serde::Serialize)]
pub struct AgentDraft {
    pub channel: String,
    pub channel_id: String,
    pub content: String,
}

/// Run a single agent turn for a participant message.
pub async fn run_turn(
    pool: &PgPool,
    agent: &AgentConfig,
    hub: &Arc<Hub>,
    ctx: &ParticipantCtx,
    user_message: &str,
    current_task_id: Option<&str>,
) -> Result<AgentRunResult, ApiError> {
    let condition = ctx.condition;
    let pid = ctx.participant_id;

    // Cost guardrail: stop a participant who has already sent the cap's worth of
    // messages. Cheap count of prior invocations for this participant.
    let prior: i64 =
        sqlx::query_scalar!("SELECT COUNT(*) FROM study_events WHERE participant_id = $1 AND event_type = 'agent_invoked'", pid)
            .fetch_one(pool)
            .await
            .map_err(|e| ApiError::Internal(format!("agent quota check failed: {e}")))?
            .unwrap_or(0);
    if prior >= MAX_AGENT_MESSAGES {
        return Err(ApiError::Validation(
            "You've reached the assistant message limit for this session.".into(),
        ));
    }

    record_server_event(
        pool,
        pid,
        "agent_invoked",
        current_task_id,
        json!({
            "trigger": "user_message",
            "condition": condition.as_str(),
            "message": user_message,
            "message_length": user_message.len(),
        }),
    )
    .await;

    // 1. Context Gate — scoped assembly + proof-of-scoping log.
    let scoped = gate::assemble(pool, ctx).await?;
    record_server_event(
        pool,
        pid,
        "agent_context_sent",
        current_task_id,
        scoped.snapshot(),
    )
    .await;

    // 2. Build the message stack stable-content-first (system → context → user)
    //    so OpenAI prompt caching applies to the repeated prefix.
    let mut messages: Vec<Value> = vec![
        json!({ "role": "system", "content": system_prompt(condition) }),
        json!({ "role": "system", "content": format!("Current project context:\n{}", scoped.render_for_prompt()) }),
        json!({ "role": "user", "content": user_message }),
    ];

    let tool_defs = tools::openai_tool_defs(condition);

    let started = std::time::Instant::now();
    let mut actions: Vec<String> = Vec::new();
    let mut entities: Vec<AgentEntity> = Vec::new();
    let mut drafts: Vec<AgentDraft> = Vec::new();
    let mut total_tool_calls = 0usize;
    let mut reply = String::new();

    for _ in 0..MAX_TURNS {
        let resp = openai::chat_completion(agent, &messages, &tool_defs).await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_server_event(
                    pool,
                    pid,
                    "agent_error",
                    current_task_id,
                    json!({ "error": e.to_string() }),
                )
                .await;
                return Err(ApiError::Internal(
                    "The assistant is unavailable, please try again.".into(),
                ));
            }
        };

        if let Some(usage) = &resp.usage {
            tracing::info!(
                participant = %pid,
                model = %agent.model,
                prompt_tokens = usage.prompt_tokens,
                completion_tokens = usage.completion_tokens,
                "agent token usage"
            );
        }

        let Some(choice) = resp.choices.into_iter().next() else {
            break;
        };
        let msg = choice.message;

        if msg.tool_calls.is_empty() {
            // Final answer.
            reply = msg.content.unwrap_or_default();
            break;
        }

        // Append the assistant turn (with its tool calls) verbatim so the
        // follow-up tool messages line up by id.
        messages.push(assistant_tool_call_message(&msg.content, &msg.tool_calls));

        for call in &msg.tool_calls {
            total_tool_calls += 1;
            let args: Value = serde_json::from_str(&call.function.arguments).unwrap_or(json!({}));
            let category = tools::category_for(&call.function.name);
            let allowed = category
                .map(|c| gate::allowed_tool_categories(condition).contains(&c))
                .unwrap_or(false);

            if !allowed {
                // Should be ~never: the model was only offered allowed tools.
                record_server_event(
                    pool,
                    pid,
                    "agent_tool_rejected",
                    current_task_id,
                    json!({ "tool": call.function.name, "condition": condition.as_str() }),
                )
                .await;
                record_server_event(
                    pool,
                    pid,
                    "agent_tool_call",
                    current_task_id,
                    json!({ "tool": call.function.name, "allowed": false, "executed": false }),
                )
                .await;
                messages.push(tool_result_message(
                    &call.id,
                    "This action is not permitted.",
                ));
                continue;
            }

            match tools::execute(pool, hub, ctx, &call.function.name, &args).await {
                Ok(outcome) => {
                    // Surface the touched entity id on the tool-call event so a
                    // later user edit to the same entity is joinable post-hoc
                    // even if the live client signal is missed.
                    let (entity_kind, entity_id, entity_action) = match &outcome.entity {
                        Some(e) => (
                            Value::from(e.kind),
                            Value::from(e.id.as_str()),
                            Value::from(e.action),
                        ),
                        None => (Value::Null, Value::Null, Value::Null),
                    };
                    record_server_event(
                        pool,
                        pid,
                        "agent_tool_call",
                        current_task_id,
                        json!({
                            "tool": call.function.name,
                            "allowed": true,
                            "executed": true,
                            // A chat draft is prepared but not posted — flag it so
                            // analysis doesn't count it as a completed chat action.
                            "drafted": outcome.draft.is_some(),
                            "args_summary": args,
                            "entity_kind": entity_kind,
                            "entity_id": entity_id,
                            "entity_action": entity_action,
                        }),
                    )
                    .await;
                    if let Some(e) = outcome.entity {
                        entities.push(AgentEntity {
                            kind: e.kind.to_string(),
                            id: e.id,
                            action: e.action.to_string(),
                        });
                    }
                    if let Some(d) = outcome.draft {
                        drafts.push(AgentDraft {
                            channel: d.channel,
                            channel_id: d.channel_id,
                            content: d.content,
                        });
                    }
                    actions.push(outcome.summary);
                    messages.push(tool_result_message(&call.id, &outcome.result));
                }
                Err(e) => {
                    record_server_event(
                        pool,
                        pid,
                        "agent_tool_call",
                        current_task_id,
                        json!({
                            "tool": call.function.name,
                            "allowed": true,
                            "executed": false,
                            "error": e.to_string(),
                        }),
                    )
                    .await;
                    messages.push(tool_result_message(&call.id, &format!("Error: {e}")));
                }
            }
        }
    }

    let response_type = if total_tool_calls == 0 {
        "advice_only"
    } else if reply.is_empty() {
        "tool_calls"
    } else {
        "mixed"
    };

    record_server_event(
        pool,
        pid,
        "agent_response",
        current_task_id,
        json!({
            "latency_ms": started.elapsed().as_millis() as u64,
            "response_type": response_type,
            "num_tool_calls": total_tool_calls,
            "model": agent.model,
            "temperature": agent.temperature,
            "prompt_version": agent.prompt_version,
            "reply": reply,
        }),
    )
    .await;

    if reply.trim().is_empty() && !actions.is_empty() {
        reply = "Done.".to_string();
    }

    Ok(AgentRunResult {
        reply,
        actions,
        entities,
        drafts,
        condition: condition.as_str().to_string(),
    })
}

fn system_prompt(condition: AgentCondition) -> String {
    // Shared rules. The no-false-action rule is critical for study validity: the
    // agent must never claim an action it cannot perform (e.g. a C2 agent has no
    // GitHub-linking tool), because a fabricated "done" confounds what the agent
    // actually did versus claimed.
    let base = "You are an AI assistant embedded in Weld, a developer workspace, helping a user \
        on the PetTracker project. Be concise and practical. Use only the project context provided; \
        do not invent cards, messages, or pull requests that aren't shown. Only report an action \
        as completed if you actually performed it by calling the matching tool — never claim or \
        imply you did something you have no tool for. If the user asks for something outside your \
        abilities, say plainly that you can't do it and tell them how to do it themselves.";
    let capability = match condition {
        AgentCondition::C1 => {
            " You have NO ability to act: you cannot change the board, post messages, or link \
            anything. You can only give advice — clearly tell the user what to do and let them do \
            it. Never say you have done something; you can only recommend."
        }
        AgentCondition::C2 => {
            " You can act on the BOARD (create, move, update, assign, label, and set priority on \
            cards) and DRAFT chat messages — only by calling the provided tools. Drafting a \
            message does NOT post it: it is shown to the user to review, edit, and send \
            themselves, so phrase your confirmation as having drafted it for them (never say you \
            posted it). You CANNOT touch GitHub in any way: you cannot link cards to issues or \
            pull requests, and you cannot read GitHub. If the user asks anything involving GitHub \
            (e.g. linking a PR), tell them plainly you can't and that they'll need to do it \
            themselves — do not claim you did it. When you can act, call the tool rather than only \
            describing it, then briefly confirm what you actually did."
        }
        AgentCondition::C3 => {
            " You can act on the BOARD (create, move, update, assign, label, and set priority on \
            cards), DRAFT chat messages, and LINK cards to existing GitHub issues or pull requests \
            — only by calling the provided tools. Drafting a message does NOT post it: it is shown \
            to the user to review, edit, and send themselves, so say you drafted it for them \
            (never say you posted it). You cannot create or modify anything on GitHub itself, only \
            link cards to items that already exist. When you can act, call the tool rather than \
            only describing it, then briefly confirm what you actually did."
        }
    };
    format!("{base}{capability}")
}

fn assistant_tool_call_message(content: &Option<String>, calls: &[openai::ToolCall]) -> Value {
    let tool_calls: Vec<Value> = calls
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "type": "function",
                "function": { "name": c.function.name, "arguments": c.function.arguments }
            })
        })
        .collect();
    json!({
        "role": "assistant",
        "content": content.clone().unwrap_or_default(),
        "tool_calls": tool_calls,
    })
}

fn tool_result_message(tool_call_id: &str, content: &str) -> Value {
    json!({ "role": "tool", "tool_call_id": tool_call_id, "content": content })
}
