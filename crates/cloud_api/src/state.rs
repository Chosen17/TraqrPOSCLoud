use db::PgPool;

/// Shared app state for Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}
