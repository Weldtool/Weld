use axum::Router;
use sqlx::PgPool;
use tokio::net::TcpListener;
use weld_server::config;
use weld_server::routes;
use weld_server::state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = config::Config::from_env();
    let addr = format!("{}:{}", config.host, config.port);

    let pool = PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to the database");
    let state = AppState {
        db: pool,
        jwt_secret: config.jwt_secret,
        secure_cookies: config.secure_cookies,
    };
    let app = Router::new().merge(routes::router()).with_state(state);

    let listener = TcpListener::bind(&addr).await.unwrap();
    tracing::info!("weld-server listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
