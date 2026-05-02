use std::sync::Arc;

use crate::realtime::Hub;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub jwt_secret: String,
    pub secure_cookies: bool,
    pub hub: Arc<Hub>,
}
