#[derive(Clone)]
#[allow(dead_code)]
pub struct AppState {
    #[allow(dead_code)]
    pub db: sqlx::PgPool,
}
