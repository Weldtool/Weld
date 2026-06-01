use std::sync::Arc;

use crate::realtime::Hub;

#[derive(Clone)]
pub struct GithubConfig {
    pub app_id: u64,
    pub app_private_key_pem: Vec<u8>,
    pub webhook_secret: String,
    pub oauth_client_id: String,
    pub oauth_client_secret: String,
    pub app_slug: String,
}

/// OpenAI-backed study agent settings. Present only when an API key is
/// configured; the agent endpoint degrades cleanly when this is `None`.
#[derive(Clone)]
pub struct AgentConfig {
    pub api_key: String,
    /// Pinned dated model snapshot, held constant for the whole study.
    pub model: String,
    /// Fixed low temperature for reproducibility (CLAUDE_AGENT_BUILD.md §6).
    pub temperature: f32,
    /// Bumped whenever the system prompt or tool defs change; logged per call.
    pub prompt_version: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub jwt_secret: String,
    pub secure_cookies: bool,
    pub hub: Arc<Hub>,
    pub github: Option<Arc<GithubConfig>>,
    pub master_encryption_key: Option<String>,
    pub frontend_url: String,
    /// Global research-study flag. Gates all study behavior; off = normal product.
    pub study_mode: bool,
    /// Study agent config; `None` when no OpenAI key is set.
    pub agent: Option<Arc<AgentConfig>>,
    /// Bearer token guarding the study data-export endpoint; `None` disables it.
    pub study_export_token: Option<String>,
    /// When true, provisioning requires a valid single-use access code.
    pub study_require_access_code: bool,
}
