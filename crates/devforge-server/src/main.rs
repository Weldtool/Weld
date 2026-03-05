mod config;
mod routes;

use axum::Router;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = config::Config::from_env();
    let addr = format!("{}:{}", config.host, config.port);

    let app = Router::new().merge(routes::router());

    let listener = TcpListener::bind(&addr).await.unwrap();
    tracing::info!("devforge-server listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
