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

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub jwt_secret: String,
    pub secure_cookies: bool,
    pub hub: Arc<Hub>,
    pub github: Option<Arc<GithubConfig>>,
    pub master_encryption_key: Option<String>,
    pub frontend_url: String,
}
