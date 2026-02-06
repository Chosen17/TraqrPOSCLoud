mod auth;
mod blog;
mod device;
mod delivery_integrations;
mod docs;
mod orders;
mod profile;
mod read_model;
mod sync;
mod tenancy;
mod entitlements;
mod super_admin;
mod utils;

use sqlx::{migrate::Migrator, MySql, Pool};
use std::path::Path;

pub type DbPool = Pool<MySql>;

pub use auth::*;
pub use blog::*;
pub use device::*;
pub use delivery_integrations::*;
pub use docs::*;
pub use orders::*;
pub use profile::*;
pub use read_model::*;
pub use sync::*;
pub use tenancy::*;
pub use entitlements::*;
pub use super_admin::*;
pub use utils::{slug_from_title, user_can_access_org, user_can_access_store};

pub async fn connect(database_url: &str) -> Result<DbPool, sqlx::Error> {
    Pool::<MySql>::connect(database_url).await
}

/// Run migrations from the workspace `migrations/` directory.
/// Call this after connect when the app starts (optional; can also use `sqlx migrate run` CLI).
pub async fn run_migrations(pool: &DbPool) -> Result<(), sqlx::Error> {
    // migrations/ is at workspace root: crates/db -> ../../migrations
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let migrations_path = Path::new(&manifest_dir).join("../../migrations");
    let migrator = Migrator::new(migrations_path).await?;
    migrator.run(pool).await?;
    Ok(())
}

/// Fails if the `plans` table (and thus entitlements migrations) is missing. Call after run_migrations.
pub async fn ensure_plans_table(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1 FROM plans LIMIT 1").execute(pool).await?;
    Ok(())
}

/// Backwards-compatible alias (use DbPool).
pub type PgPool = DbPool;
