//! Thin OpenAI Chat Completions client (CLAUDE_AGENT_BUILD.md §6). Isolated
//! behind this module so swapping models/providers is a one-file change. The
//! model is pinned and the temperature fixed by `AgentConfig`; token usage is
//! returned so the caller can log spend.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::errors::ApiError;
use crate::state::AgentConfig;

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const MAX_OUTPUT_TOKENS: u32 = 800;

// Request-shape differences between model generations, learned from the API's
// own rejections and cached for the life of the process so only the first call
// pays for the correction. Two parameters have moved:
//
//   * the output-token cap is `max_tokens` on older snapshots and
//     `max_completion_tokens` on newer ones — we default to the newer spelling
//     because the pinned study model is a newer snapshot;
//   * `temperature` is not accepted at all by some reasoning snapshots, which
//     run at a fixed default instead.
//
// Guessing wrong in either direction is a hard 400 that kills the agent for
// every participant, and which one is right can't be settled without calling
// the live API — so the client asks the API and corrects itself.
static USE_MAX_COMPLETION_TOKENS: AtomicBool = AtomicBool::new(true);
static SEND_TEMPERATURE: AtomicBool = AtomicBool::new(true);
/// Flip the token parameter at most once, so a confusing error can't put the
/// client in a loop swapping between two spellings that both fail.
static TOKEN_PARAM_ADAPTED: AtomicBool = AtomicBool::new(false);

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

/// The temperature actually in effect: the configured value, or `None` once the
/// API has told us this snapshot doesn't accept one (it then runs at the
/// provider's default). Recorded per response so the methods section's
/// determinism claim can be checked against the collected data rather than
/// assumed.
pub fn effective_temperature(agent: &AgentConfig) -> Option<f32> {
    SEND_TEMPERATURE
        .load(Ordering::Relaxed)
        .then_some(agent.temperature)
}

fn build_body(agent: &AgentConfig, messages: &[Value], tools: &[Value]) -> Value {
    let mut body = json!({
        "model": agent.model,
        "messages": messages,
    });
    let token_field = if USE_MAX_COMPLETION_TOKENS.load(Ordering::Relaxed) {
        "max_completion_tokens"
    } else {
        "max_tokens"
    };
    body[token_field] = json!(MAX_OUTPUT_TOKENS);
    if SEND_TEMPERATURE.load(Ordering::Relaxed) {
        body["temperature"] = json!(agent.temperature);
    }
    if !tools.is_empty() {
        body["tools"] = json!(tools);
        body["tool_choice"] = json!("auto");
    }
    body
}

/// One Chat Completions call. `messages` is the running conversation (system →
/// context → user, then assistant/tool turns). `tools` is the condition's
/// allowed function set (empty for C1 → advise-only).
///
/// Retries once on a transient failure (transport, 429, 5xx). A deterministic
/// rejection surfaces immediately, *except* when the API names a request
/// parameter it doesn't support — that one is correctable, so the shape is
/// fixed, remembered process-wide, and the call retried.
pub async fn chat_completion(
    agent: &AgentConfig,
    messages: &[Value],
    tools: &[Value],
) -> Result<ChatResponse, ApiError> {
    let client = reqwest::Client::new();
    let mut last_err: Option<ApiError> = None;
    let mut transport_attempts = 0;

    // Two transport attempts plus room for the two possible shape corrections.
    for _ in 0..4 {
        let body = build_body(agent, messages, tools);
        match send(&client, agent, &body).await {
            Ok(resp) => return Ok(resp),
            Err(SendError::Retryable(e)) => {
                last_err = Some(e);
                transport_attempts += 1;
                if transport_attempts >= 2 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            }
            Err(SendError::Permanent { err, body }) => {
                // A rejected request shape fails identically every time, so
                // retrying it verbatim only doubles the participant's wait. But
                // the API names the offending parameter: if we can correct the
                // shape, retry immediately with the correction.
                if !adapt_request_shape(&body) {
                    return Err(err);
                }
                last_err = Some(err);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| ApiError::Internal("OpenAI call failed".into())))
}

/// Apply a correction implied by an error body. Returns whether anything
/// changed — i.e. whether retrying is worth a second call.
fn adapt_request_shape(body: &str) -> bool {
    match rejected_param(body) {
        // Only ever one of the two token fields is sent, so a complaint naming
        // either means the one we chose is the wrong one for this snapshot.
        Some(RejectedParam::OutputTokens) if !TOKEN_PARAM_ADAPTED.swap(true, Ordering::Relaxed) => {
            let use_new = !USE_MAX_COMPLETION_TOKENS.load(Ordering::Relaxed);
            USE_MAX_COMPLETION_TOKENS.store(use_new, Ordering::Relaxed);
            tracing::warn!(
                field = if use_new {
                    "max_completion_tokens"
                } else {
                    "max_tokens"
                },
                "OpenAI rejected the output-token parameter; retrying with the other spelling"
            );
            true
        }
        Some(RejectedParam::Temperature) if SEND_TEMPERATURE.swap(false, Ordering::Relaxed) => {
            tracing::warn!(
                "OpenAI rejected the configured temperature; retrying without it. This model \
                 snapshot runs at the provider default — the study is NOT at the configured \
                 temperature and the methods section must say so."
            );
            true
        }
        _ => false,
    }
}

/// The request parameter an error body blames, when it is one we can correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RejectedParam {
    OutputTokens,
    Temperature,
}

/// Read the blamed parameter out of an OpenAI error body. The structured
/// `error.param` field is authoritative; the message text is a fallback for
/// snapshots that only spell the parameter out in prose.
fn rejected_param(body: &str) -> Option<RejectedParam> {
    let parsed = serde_json::from_str::<Value>(body).ok();
    let err = parsed.as_ref().and_then(|v| v.get("error"));
    let param = err.and_then(|e| e.get("param")).and_then(Value::as_str);
    let message = err
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or(body);

    let names = |needle: &str| param == Some(needle) || message.contains(needle);

    // Checked first: the "use max_completion_tokens instead" message names both
    // token fields, and one of those substrings must not be read as something
    // else.
    if names("max_completion_tokens") || names("max_tokens") {
        return Some(RejectedParam::OutputTokens);
    }
    if names("temperature") {
        return Some(RejectedParam::Temperature);
    }
    None
}

/// Whether a failed call is worth a second attempt. Transport faults, rate
/// limits and 5xx are transient; every other 4xx is a deterministic rejection,
/// whose body is kept so the caller can look for a correctable parameter.
enum SendError {
    Retryable(ApiError),
    Permanent { err: ApiError, body: String },
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
                SendError::Permanent { err, body: text }
            },
        );
    }

    serde_json::from_str::<ChatResponse>(&text).map_err(|e| {
        SendError::Retryable(ApiError::Internal(format!(
            "OpenAI response parse failed: {e}"
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real 400 a GPT-5-era snapshot returns for `max_tokens`. It names
    /// both spellings, so the fallback must not be fooled by the prose.
    #[test]
    fn detects_the_output_token_rename() {
        let body = r#"{"error":{"message":"Unsupported parameter: 'max_tokens' is not supported with this model. Use 'max_completion_tokens' instead.","type":"invalid_request_error","param":"max_tokens","code":"unsupported_parameter"}}"#;
        assert_eq!(rejected_param(body), Some(RejectedParam::OutputTokens));
    }

    /// The other direction: an older snapshot rejecting the newer spelling.
    #[test]
    fn detects_the_output_token_rename_in_reverse() {
        let body = r#"{"error":{"message":"Unrecognized request argument supplied: max_completion_tokens","type":"invalid_request_error","param":null,"code":null}}"#;
        assert_eq!(rejected_param(body), Some(RejectedParam::OutputTokens));
    }

    #[test]
    fn detects_an_unsupported_temperature() {
        let body = r#"{"error":{"message":"Unsupported value: 'temperature' does not support 0.2 with this model. Only the default (1) value is supported.","type":"invalid_request_error","param":"temperature","code":"unsupported_value"}}"#;
        assert_eq!(rejected_param(body), Some(RejectedParam::Temperature));
    }

    /// A bad key or a missing model is not a shape problem — there is nothing to
    /// correct, so the caller must surface it instead of burning a second call.
    #[test]
    fn leaves_uncorrectable_errors_alone() {
        let unauthorized = r#"{"error":{"message":"Incorrect API key provided.","type":"invalid_request_error","param":null,"code":"invalid_api_key"}}"#;
        assert_eq!(rejected_param(unauthorized), None);

        let no_model = r#"{"error":{"message":"The model 'gpt-9' does not exist","type":"invalid_request_error","param":"model","code":"model_not_found"}}"#;
        assert_eq!(rejected_param(no_model), None);
    }

    /// Non-JSON bodies (a proxy's HTML error page) must not panic the client.
    #[test]
    fn survives_a_non_json_body() {
        assert_eq!(rejected_param("<html>502 Bad Gateway</html>"), None);
        assert_eq!(rejected_param(""), None);
    }
}
