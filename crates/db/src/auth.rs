//! Portal login: lookup user by email, verify password with pgcrypto crypt.

use sqlx::PgPool;
use uuid::Uuid;

/// User row returned on successful login.
#[derive(Debug, sqlx::FromRow)]
pub struct LoginUserRow {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
}

/// Verify email + password. Uses pgcrypto: password_hash in DB is from crypt(plain, gen_salt('bf')).
/// Returns user row if password matches.
pub async fn verify_login(
    pool: &PgPool,
    email: &str,
    password: &str,
) -> Result<Option<LoginUserRow>, sqlx::Error> {
    let row = sqlx::query_as(
        r#"
        SELECT id, email, display_name
        FROM cloud_users
        WHERE email = $1 AND status = 'active' AND password_hash IS NOT NULL
          AND password_hash = crypt($2, password_hash)
        "#,
    )
    .bind(email)
    .bind(password)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Update last_login_at for user.
pub async fn update_last_login(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE cloud_users SET last_login_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
