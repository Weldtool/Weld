#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    pub db: sqlx::PgPool,
}
