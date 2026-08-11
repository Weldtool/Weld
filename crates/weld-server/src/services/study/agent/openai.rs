//! Thin OpenAI Chat Completions client (CLAUDE_AGENT_BUILD.md §6). Isolated
//! behind this module so swapping models/providers is a one-file change. The
//! model is pinned and the temperature fixed by `AgentConfig`; token usage is
//! returned so the caller can log spend.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::errors::ApiError;
use crate::state::AgentConfig;

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const MAX_TOKENS: u32 = 800;

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: RespMessage,
}

#[derive(Debug, Deserialize)]
pub struct RespMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    /// JSON-encoded argument object (per the OpenAI tool-calling contract).
    pub arguments: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: i64,
    #[serde(default)]
    pub completion_tokens: i64,
    #[serde(default)]
    pub total_tokens: i64,
}

/// One Chat Completions call. `messages` is the running conversation (system →
/// context → user, then assistant/tool turns). `tools` is the condition's
/// allowed function set (empty for C1 → advise-only). One retry on a transient
/// failure (transport, 429, 5xx); a deterministic rejection (any other 4xx)
/// surfaces on the first attempt.
pub async fn chat_completion(
    agent: &AgentConfig,
    messages: &[Value],
    tools: &[Value],
) -> Result<ChatResponse, ApiError> {
    let mut body = json!({
        "model": agent.model,
        "temperature": agent.temperature,
        "max_tokens": MAX_TOKENS,
        "messages": messages,
    });
    if !tools.is_empty() {
        body["tools"] = json!(tools);
        body["tool_choice"] = json!("auto");
    }

    let client = reqwest::Client::new();
    let mut last_err: Option<ApiError> = None;
    for attempt in 0..2 {
        match send(&client, agent, &body).await {
            Ok(resp) => return Ok(resp),
            Err(SendError::Retryable(e)) => {
                last_err = Some(e);
                if attempt == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                }
            }
            // A rejected request shape (e.g. an unsupported `max_tokens` or
            // `temperature` on a newer snapshot) fails identically every time.
            // Retrying it doubles the participant's wait for the same error, so
            // surface it immediately.
            Err(SendError::Permanent(e)) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| ApiError::Internal("OpenAI call failed".into())))
}

/// Whether a failed call is worth a second attempt. Transport faults, rate
/// limits and 5xx are transient; every other 4xx is a deterministic rejection.
enum SendError {
    Retryable(ApiError),
    Permanent(ApiError),
}

async fn send(
    client: &reqwest::Client,
    agent: &AgentConfig,
    body: &Value,
) -> Result<ChatResponse, SendError> {
    let resp = client
        .post(ENDPOINT)
        .bearer_auth(&agent.api_key)
        .json(body)
        .send()
        .await
        .map_err(|e| {
            SendError::Retryable(ApiError::Internal(format!("OpenAI request failed: {e}")))
        })?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| {
        SendError::Retryable(ApiError::Internal(format!(
            "OpenAI response read failed: {e}"
        )))
    })?;

    if !status.is_success() {
        let err = ApiError::Internal(format!("OpenAI returned {status}: {text}"));
        return Err(
            if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                SendError::Retryable(err)
            } else {
                SendError::Permanent(err)
            },
        );
    }

    serde_json::from_str::<ChatResponse>(&text).map_err(|e| {
        SendError::Retryable(ApiError::Internal(format!(
            "OpenAI response parse failed: {e}"
        )))
    })
}
