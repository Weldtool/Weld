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
        // Record the refusal, but NOT as `agent_invoked` — that event is the
        // denominator of the RQ2 invocation counts (and of this very quota), so
        // a turn that never reached the model must not inflate it.
        record_server_event(
            pool,
            pid,
            "agent_error",
            current_task_id,
            json!({ "error": "quota_exceeded", "prior_invocations": prior }),
        )
        .await;
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

    // 2. Load prior turns for this task from the DB (server-owned; the client
    //    never controls what enters the LLM context).
    let task_key = current_task_id.unwrap_or("__notask__");
    let prior_turns = sqlx::query!(
        r#"SELECT role, content FROM study_agent_turns
           WHERE participant_id = $1 AND task_id = $2
           ORDER BY created_at ASC"#,
        ctx.participant_id,
        task_key,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("failed to load conversation history: {e}")))?;

    // 3. Build the message stack stable-content-first (system → context →
    //    history → user) so OpenAI prompt caching applies to the repeated prefix.
    let mut messages: Vec<Value> = vec![
        json!({ "role": "system", "content": system_prompt(condition) }),
        json!({ "role": "system", "content": format!("Current project context:\n{}", scoped.render_for_prompt()) }),
    ];
    for turn in &prior_turns {
        messages.push(json!({ "role": turn.role, "content": turn.content }));
    }
    messages.push(json!({ "role": "user", "content": user_message }));

    let tool_defs = tools::openai_tool_defs(condition);

    let started = std::time::Instant::now();
    let mut actions: Vec<String> = Vec::new();
    let mut entities: Vec<AgentEntity> = Vec::new();
    let mut drafts: Vec<AgentDraft> = Vec::new();
    let mut total_tool_calls = 0usize;
    let mut reply = String::new();
    // Accumulate token usage across all model calls in this turn (a turn may
    // make several OpenAI calls when the model iterates on tool results) so the
    // agent_response event carries the full per-turn cost for analysis.
    let mut prompt_tokens = 0i64;
    let mut completion_tokens = 0i64;
    let mut total_tokens = 0i64;

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
                return Err(ApiError::Unavailable(
                    "The assistant is unavailable, please try again.".into(),
                ));
            }
        };

        if let Some(usage) = &resp.usage {
            prompt_tokens += usage.prompt_tokens;
            completion_tokens += usage.completion_tokens;
            total_tokens += usage.total_tokens;
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

    // Post-processing safety net: the system prompt tells the model to refer to
    // cards by title, but an LLM can still slip and echo a raw card id. Enforce
    // the rule deterministically — swap any leaked card id for the card's quoted
    // title, redact any other stray UUID — so a participant never sees an opaque
    // id even on a model miss. Log the catch so we can audit how often it fires.
    let leaks = {
        let (sanitized, n) = sanitize_card_ids(&reply, scoped.board.as_deref());
        if n > 0 {
            reply = sanitized;
        }
        n
    };
    if leaks > 0 {
        record_server_event(
            pool,
            pid,
            "agent_id_leak_sanitized",
            current_task_id,
            json!({ "count": leaks }),
        )
        .await;
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
            "temperature": openai::effective_temperature(agent),
            "prompt_version": agent.prompt_version,
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens,
            "reply": reply,
        }),
    )
    .await;

    // Substituted only after `agent_response` is recorded above, so telemetry
    // keeps what the model actually returned.
    if reply.trim().is_empty() {
        reply = if actions.is_empty() {
            // The model produced nothing (hit the output cap, or exhausted
            // MAX_TURNS). An empty bubble reads as a broken UI, and an empty
            // assistant turn replayed as history confuses the next one.
            "I wasn't able to put together a reply to that. Could you try rephrasing?".to_string()
        } else {
            "Done.".to_string()
        };
    }

    // Persist both sides of this exchange so future turns have server-owned
    // history to replay. Failures are non-fatal — a missed row means the next
    // turn loses one round of context, which is better than surfacing a DB
    // error to the participant.
    let _ = sqlx::query!(
        "INSERT INTO study_agent_turns (participant_id, task_id, role, content)
         VALUES ($1, $2, 'user', $3), ($1, $2, 'assistant', $4)",
        ctx.participant_id,
        task_key,
        user_message,
        reply,
    )
    .execute(pool)
    .await;

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
        abilities, say plainly that you can't do it and tell them how to do it themselves. \
        Always refer to a card by its title in quotes (e.g. \"Fix login bug\"), never by its id. \
        The id shown next to each card is an internal identifier for calling tools only — never \
        show it to the user or mention it in your replies.";
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

/// Deterministic backstop for the "refer to cards by title, never by id" rule.
/// Scans `reply` for UUID-shaped tokens: a known card id is replaced with the
/// card's quoted title, any other stray UUID is redacted to "that card" (in the
/// scoped study context, card ids are the only UUIDs the model is ever shown).
/// Returns the rewritten reply and the number of substitutions made.
fn sanitize_card_ids(reply: &str, board: Option<&[gate::CardLine]>) -> (String, usize) {
    let titles: std::collections::HashMap<String, String> = board
        .unwrap_or(&[])
        .iter()
        .map(|c| (c.id.to_string(), format!("\"{}\"", c.title)))
        .collect();

    let mut out = String::with_capacity(reply.len());
    let mut count = 0usize;
    let mut i = 0;
    while i < reply.len() {
        if let Some(uuid) = uuid_at(reply, i) {
            match titles.get(&uuid.to_ascii_lowercase()) {
                Some(title) => out.push_str(title),
                None => out.push_str("that card"),
            }
            count += 1;
            i += uuid.len();
            continue;
        }
        // Safe: we only advance `i` by a full UUID (36 ASCII bytes) or a full
        // char, so `i` is always on a char boundary here.
        let ch = reply[i..].chars().next().expect("char boundary");
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, count)
}

/// If a canonical 36-char UUID (8-4-4-4-12 hex) begins at byte `i` and is not
/// part of a longer hex/hyphen run on either side, return its slice.
fn uuid_at(s: &str, i: usize) -> Option<&str> {
    const HYPHENS: [usize; 4] = [8, 13, 18, 23];
    let b = s.as_bytes();
    if i + 36 > b.len() {
        return None;
    }
    for (pos, &c) in b[i..i + 36].iter().enumerate() {
        if HYPHENS.contains(&pos) {
            if c != b'-' {
                return None;
            }
        } else if !c.is_ascii_hexdigit() {
            return None;
        }
    }
    // Reject if it's the middle of a longer token (e.g. a 40-char hash).
    if i > 0 && (b[i - 1].is_ascii_hexdigit() || b[i - 1] == b'-') {
        return None;
    }
    if i + 36 < b.len() && (b[i + 36].is_ascii_hexdigit() || b[i + 36] == b'-') {
        return None;
    }
    Some(&s[i..i + 36])
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn card(id: Uuid, title: &str) -> gate::CardLine {
        gate::CardLine {
            id,
            title: title.to_string(),
            description: None,
            column: "To Do".into(),
            assignee: None,
            priority: "medium".into(),
            labels: vec![],
            github_issue: None,
            github_pr: None,
        }
    }

    #[test]
    fn known_card_id_is_replaced_with_quoted_title() {
        let id = Uuid::parse_str("3f2a1b4c-0000-4000-8000-000000000001").unwrap();
        let board = vec![card(id, "Fix login bug")];
        let reply = format!("You should look at card {id} next.");
        let (out, n) = sanitize_card_ids(&reply, Some(&board));
        assert_eq!(out, "You should look at card \"Fix login bug\" next.");
        assert_eq!(n, 1);
    }

    #[test]
    fn uppercased_card_id_still_matches() {
        let id = Uuid::parse_str("3f2a1b4c-0000-4000-8000-000000000001").unwrap();
        let board = vec![card(id, "Fix login bug")];
        let reply = format!("card {} is ready", id.to_string().to_uppercase());
        let (out, n) = sanitize_card_ids(&reply, Some(&board));
        assert_eq!(out, "card \"Fix login bug\" is ready");
        assert_eq!(n, 1);
    }

    #[test]
    fn unknown_uuid_is_redacted() {
        let reply = "see 11111111-2222-4333-8444-555555555555 for details";
        let (out, n) = sanitize_card_ids(reply, Some(&[]));
        assert_eq!(out, "see that card for details");
        assert_eq!(n, 1);
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let reply = "Move \"Fix login bug\" to Done and ping the team.";
        let (out, n) = sanitize_card_ids(reply, Some(&[]));
        assert_eq!(out, reply);
        assert_eq!(n, 0);
    }

    #[test]
    fn longer_hex_run_is_not_treated_as_uuid() {
        // A 40-char git-style hash must not be mistaken for an embedded UUID.
        let reply = "commit 3f2a1b4c000040008000000000000001abcdef01 landed";
        let (out, n) = sanitize_card_ids(reply, Some(&[]));
        assert_eq!(out, reply);
        assert_eq!(n, 0);
    }
}
