use std::sync::Arc;

use axum::Router;
use base64::{Engine as _, engine::general_purpose};
use sqlx::PgPool;
use tokio::net::TcpListener;
use weld_server::config;
use weld_server::realtime::Hub;
use weld_server::routes;
use weld_server::state::{AgentConfig, AppState, GithubConfig};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = config::Config::from_env();
    let addr = format!("{}:{}", config.host, config.port);

    let pool = PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to the database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    let github = build_github_config(&config);
    let agent = config.openai_api_key.clone().map(|api_key| {
        Arc::new(AgentConfig {
            api_key,
            model: config.openai_model.clone(),
            temperature: 0.2,
            prompt_version: "v1".to_string(),
        })
    });

    let state = AppState {
        db: pool,
        jwt_secret: config.jwt_secret,
        secure_cookies: config.secure_cookies,
        hub: Arc::new(Hub::new()),
        github,
        master_encryption_key: config.master_encryption_key,
        frontend_url: config.frontend_url,
        study_mode: config.study_mode,
        agent,
    };

    if state.study_mode {
        tracing::warn!("STUDY_MODE is ON — research-study behavior is active");
    }

    let app = Router::new().merge(routes::router()).with_state(state);

    let listener = TcpListener::bind(&addr).await.unwrap();
    tracing::info!("weld-server listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}

fn build_github_config(config: &config::Config) -> Option<Arc<GithubConfig>> {
    let app_id_str = config.github_app_id.as_deref()?;
    let app_id: u64 = app_id_str.parse().expect("GITHUB_APP_ID must be a number");

    let pem_b64 = config.github_app_private_key.as_deref()?;
    let app_private_key_pem = general_purpose::STANDARD
        .decode(pem_b64)
        .expect("GITHUB_APP_PRIVATE_KEY must be valid base64");

    let webhook_secret = config.github_webhook_secret.clone()?;
    let oauth_client_id = config.github_oauth_client_id.clone()?;
    let oauth_client_secret = config.github_oauth_client_secret.clone()?;
    let app_slug = config.github_app_slug.clone()?;

    Some(Arc::new(GithubConfig {
        app_id,
        app_private_key_pem,
        webhook_secret,
        oauth_client_id,
        oauth_client_secret,
        app_slug,
    }))
}
