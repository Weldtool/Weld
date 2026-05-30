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
/// allowed function set (empty for C1 → advise-only). One retry on transport
/// error; API errors surface cleanly.
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
            Err(e) => {
                last_err = Some(e);
                if attempt == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| ApiError::Internal("OpenAI call failed".into())))
}

async fn send(
    client: &reqwest::Client,
    agent: &AgentConfig,
    body: &Value,
) -> Result<ChatResponse, ApiError> {
    let resp = client
        .post(ENDPOINT)
        .bearer_auth(&agent.api_key)
        .json(body)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("OpenAI request failed: {e}")))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| ApiError::Internal(format!("OpenAI response read failed: {e}")))?;

    if !status.is_success() {
        return Err(ApiError::Internal(format!(
            "OpenAI returned {status}: {text}"
        )));
    }

    serde_json::from_str::<ChatResponse>(&text)
        .map_err(|e| ApiError::Internal(format!("OpenAI response parse failed: {e}")))
}
