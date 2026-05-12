pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub secure_cookies: bool,
    pub frontend_url: String,
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
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
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
