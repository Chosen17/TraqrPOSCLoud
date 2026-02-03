use db::PgPool;

/// Shared app state for Axum handlers. DB is optional so the server can start and serve the web UI when Postgres is not running.
#[derive(Clone)]
pub struct AppState {
    pub db: Option<PgPool>,
}
