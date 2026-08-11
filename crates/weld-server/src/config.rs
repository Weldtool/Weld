pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub secure_cookies: bool,
    pub frontend_url: String,
    /// When true, the server runs in research-study mode (see `paper/instrument/`):
    /// study provisioning/telemetry/agent endpoints are active, GitHub writes are
    /// locked down, and the frontend renders the study flow. Off = normal product.
    pub study_mode: bool,
    // Study agent (OpenAI). Only used under study_mode; the agent endpoint
    // degrades cleanly if the key is unset.
    pub openai_api_key: Option<String>,
    /// Pinned, dated model snapshot — a mid-study change is a confound, so this
    /// is held constant for the whole study (CLAUDE_AGENT_BUILD.md §6).
    ///
    /// Capability matters to the design, not just to cost: the C3 arm can only
    /// convert its wider context into performance if the model is strong enough
    /// to use it. A weak model floors exactly the condition the study is about
    /// and yields a null that says nothing about context breadth. Verify this
    /// snapshot is still served before a run and record the exact string in the
    /// paper's methods.
    pub openai_model: String,
    /// Bearer token guarding the study data-export endpoint. When unset the
    /// endpoint is disabled (404), so exports are off unless explicitly enabled.
    pub study_export_token: Option<String>,
    /// When true, provisioning requires a valid, unused single-use access code,
    /// capping participation to one run per recruited person. Off (default) lets
    /// provisioning run without a code, which keeps local testing frictionless.
    pub study_require_access_code: bool,
    pub master_encryption_key: Option<String>,
    // GitHub App integration (all optional; set in .env to enable)
    pub github_app_id: Option<String>,
    pub github_app_private_key: Option<String>,
    pub github_webhook_secret: Option<String>,
    pub github_oauth_client_id: Option<String>,
    pub github_oauth_client_secret: Option<String>,
    pub github_app_slug: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse::<u16>()
                .expect("PORT must be a valid u16"),
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: std::env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            secure_cookies: std::env::var("SECURE_COOKIES")
                .unwrap_or_else(|_| "false".to_string())
                .parse::<bool>()
                .unwrap_or(false),
            frontend_url: std::env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            study_mode: std::env::var("STUDY_MODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse::<bool>()
                .unwrap_or(false),
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            openai_model: std::env::var("OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-5.4-2026-03-05".to_string()),
            study_export_token: std::env::var("STUDY_EXPORT_TOKEN").ok(),
            study_require_access_code: std::env::var("STUDY_REQUIRE_ACCESS_CODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse::<bool>()
                .unwrap_or(false),
            master_encryption_key: std::env::var("MASTER_ENCRYPTION_KEY").ok(),
            github_app_id: std::env::var("GITHUB_APP_ID").ok(),
            github_app_private_key: std::env::var("GITHUB_APP_PRIVATE_KEY").ok(),
            github_webhook_secret: std::env::var("GITHUB_WEBHOOK_SECRET").ok(),
            github_oauth_client_id: std::env::var("GITHUB_OAUTH_CLIENT_ID").ok(),
            github_oauth_client_secret: std::env::var("GITHUB_OAUTH_CLIENT_SECRET").ok(),
            github_app_slug: std::env::var("GITHUB_APP_SLUG").ok(),
        }
    }
}
