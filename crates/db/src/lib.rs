use sqlx::{Pool, Postgres};

pub type PgPool = Pool<Postgres>;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    Pool::<Postgres>::connect(database_url).await
}
