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

pub struct AgentRunResult {
    /// The agent's final text reply to show the user.
    pub reply: String,
    /// Attributable summaries of actions the agent performed (for the UI).
    pub actions: Vec<String>,
    pub condition: String,
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

    record_server_event(
        pool,
        pid,
        "agent_invoked",
        current_task_id,
        json!({ "trigger": "user_message", "condition": condition.as_str() }),
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
                    record_server_event(
                        pool,
                        pid,
                        "agent_tool_call",
                        current_task_id,
                        json!({
                            "tool": call.function.name,
                            "allowed": true,
                            "executed": true,
                            "args_summary": args,
                        }),
                    )
                    .await;
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
        }),
    )
    .await;

    if reply.trim().is_empty() && !actions.is_empty() {
        reply = "Done.".to_string();
    }

    Ok(AgentRunResult {
        reply,
        actions,
        condition: condition.as_str().to_string(),
    })
}

fn system_prompt(condition: AgentCondition) -> String {
    let base = "You are an AI assistant embedded in Weld, a developer workspace, helping a user \
        on the PetTracker project. Be concise and practical. Use only the project context provided; \
        do not invent cards, messages, or pull requests that aren't shown.";
    let capability = match condition {
        AgentCondition::C1 => {
            " You can only give advice — you have no ability to act. Tell the user clearly what to do."
        }
        AgentCondition::C2 | AgentCondition::C3 => {
            " When the user asks you to do something you have a tool for, call the tool rather than \
            only describing it. After acting, briefly confirm what you did."
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
