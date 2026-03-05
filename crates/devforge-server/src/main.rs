mod config;
mod routes;
mod state;
use crate::state::AppState;
use axum::Router;
use sqlx::PgPool;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = config::Config::from_env();
    let addr = format!("{}:{}", config.host, config.port);

    let pool = PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to the database");
    let state = AppState { db: pool };
    let app = Router::new().merge(routes::router()).with_state(state);

    let listener = TcpListener::bind(&addr).await.unwrap();
    tracing::info!("devforge-server listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
