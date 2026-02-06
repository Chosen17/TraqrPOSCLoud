//! Helpers for super-admin / internal control-plane features.
//!
//! We treat `super_admin` and Traqr-internal roles (`sa_owner`, `sa_manager`, `sa_sales_rep`)
//! as having full access across all organizations (e.g. super-admin customer list and detail).
//! This is enforced at the API layer by checking the caller's user id against this helper.

use sqlx::MySqlPool;

/// Role codes that grant super-admin (full org) access. Must match dashboard "Traqr team" visibility.
const SUPER_ADMIN_ROLE_CODES: &[&str] = &["super_admin", "sa_owner", "sa_manager", "sa_sales_rep"];

/// Returns true if the given user has a role that grants super-admin access (e.g. super_admin or sa_owner).
pub async fn is_super_admin(pool: &MySqlPool, user_id: &str) -> Result<bool, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT r.code
        FROM org_memberships om
        JOIN cloud_roles r ON r.id = om.role_id
        WHERE om.user_id = ? AND om.status = 'active'
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let has_role = rows.iter().any(|(code,)| SUPER_ADMIN_ROLE_CODES.contains(&code.as_str()));
    Ok(has_role)
}

