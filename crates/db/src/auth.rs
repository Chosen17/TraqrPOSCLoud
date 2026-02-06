//! Portal login: lookup user by email, verify password with bcrypt.

use sqlx::MySqlPool;
use uuid::Uuid;

/// User row returned on successful login.
#[derive(Debug, sqlx::FromRow)]
pub struct LoginUserRow {
    /// Stored as CHAR(36) in MySQL; we keep it as String to avoid UUID/BINARY(16) mismatch.
    pub id: String,
    pub email: String,
    pub display_name: Option<String>,
}

/// Verify email + password. Password_hash in DB is bcrypt. Returns user row if password matches.
pub async fn verify_login(
    pool: &MySqlPool,
    email: &str,
    password: &str,
) -> Result<Option<LoginUserRow>, sqlx::Error> {
    let row: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, email, display_name, password_hash
        FROM cloud_users
        WHERE LOWER(email) = LOWER(?) AND status = 'active' AND password_hash IS NOT NULL
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    let Some((id, email_val, display_name, password_hash)) = row else {
        return Ok(None);
    };
    let Some(hash) = password_hash else {
        return Ok(None);
    };
    // Trim in case DB/MySQL returned hash with trailing newline or whitespace
    let hash = hash.trim();
    if !bcrypt::verify(password, hash).unwrap_or(false) {
        return Ok(None);
    }
    Ok(Some(LoginUserRow {
        id,
        email: email_val,
        display_name,
    }))
}

/// Update last_login_at for user.
pub async fn update_last_login(pool: &MySqlPool, user_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE cloud_users SET last_login_at = NOW() WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Role code for a user in the Traqr Internal org (sa_owner, sa_manager, sa_sales_rep).
/// Returns None if user is not a member; maps super_admin to sa_owner for backward compat.
pub async fn get_traqr_internal_role(pool: &MySqlPool, user_id: &str) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT r.code
        FROM org_memberships om
        JOIN organizations o ON o.id = om.org_id AND o.slug = 'traqr-internal'
        JOIN cloud_roles r ON r.id = om.role_id
        WHERE om.user_id = ? AND om.status = 'active'
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let code = row.map(|r| r.0);
    Ok(code.map(|c| if c == "super_admin" { "sa_owner".to_string() } else { c }))
}

/// Create a session for the user; returns (session_id, token). Caller sets cookie.
pub async fn create_session(
    pool: &MySqlPool,
    user_id: &str,
    ttl_secs: i64,
) -> Result<(String, String), sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let token = uuid::Uuid::new_v4().to_string().replace('-', "");
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl_secs);
    sqlx::query(
        "INSERT INTO cloud_sessions (id, user_id, token, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&token)
    .bind(expires_at.naive_utc())
    .execute(pool)
    .await?;
    Ok((id, token))
}

/// Delete session by token (logout).
pub async fn delete_session_by_token(pool: &MySqlPool, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM cloud_sessions WHERE token = ? AND expires_at > CURRENT_TIMESTAMP(3)")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Return user_id if token is valid and not expired.
pub async fn get_user_id_by_session_token(
    pool: &MySqlPool,
    token: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT user_id FROM cloud_sessions WHERE token = ? AND expires_at > CURRENT_TIMESTAMP(3)",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Create a new cloud user with hashed password. Returns user_id.
pub async fn create_cloud_user(
    pool: &MySqlPool,
    email: &str,
    password: &str,
    display_name: Option<&str>,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| {
        sqlx::Error::Protocol(format!("bcrypt hash error: {}", e).into())
    })?;
    sqlx::query(
        r#"
        INSERT INTO cloud_users (id, email, password_hash, display_name, status)
        VALUES (?, ?, ?, ?, 'active')
        "#,
    )
    .bind(&id)
    .bind(email)
    .bind(hash)
    .bind(display_name)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Look up cloud_roles.id by code (e.g. "head_office_admin").
pub async fn get_role_id_by_code(
    pool: &MySqlPool,
    code: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM cloud_roles WHERE code = ?")
            .bind(code)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

/// Create an org_membership row for the user and org with the given role code.
pub async fn add_org_membership(
    pool: &MySqlPool,
    org_id: Uuid,
    user_id: &str,
    role_code: &str,
) -> Result<(), sqlx::Error> {
    if let Some(role_id) = get_role_id_by_code(pool, role_code).await? {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO org_memberships (id, org_id, user_id, franchise_id, role_id, status)
            VALUES (?, ?, ?, NULL, ?, 'active')
            "#,
        )
        .bind(&id)
        .bind(org_id.to_string())
        .bind(user_id)
        .bind(role_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Create a store_membership row for the user and store with the given role code.
pub async fn add_store_membership(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    user_id: &str,
    role_code: &str,
) -> Result<(), sqlx::Error> {
    if let Some(role_id) = get_role_id_by_code(pool, role_code).await? {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO store_memberships (id, org_id, store_id, user_id, role_id, status)
            VALUES (?, ?, ?, ?, ?, 'active')
            "#,
        )
        .bind(&id)
        .bind(org_id.to_string())
        .bind(store_id.to_string())
        .bind(user_id)
        .bind(role_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}
