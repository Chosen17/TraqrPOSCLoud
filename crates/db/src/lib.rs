mod auth;
mod device;
mod sync;

use sqlx::{migrate::Migrator, Pool, Postgres};
use std::path::Path;

pub type PgPool = Pool<Postgres>;

pub use auth::*;
pub use device::*;
pub use sync::*;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    Pool::<Postgres>::connect(database_url).await
}

/// Run migrations from the workspace `migrations/` directory.
/// Call this after connect when the app starts (optional; can also use `sqlx migrate run` CLI).
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    // migrations/ is at workspace root: crates/db -> ../../migrations
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let migrations_path = Path::new(&manifest_dir).join("../../migrations");
    let migrator = Migrator::new(migrations_path).await?;
    migrator.run(pool).await?;
    Ok(())
}
