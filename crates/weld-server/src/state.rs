#[derive(Clone)]
#[allow(dead_code)]
pub struct AppState {
    pub db: sqlx::PgPool,
}
