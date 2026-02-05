//! Helpers for super-admin / internal control-plane features.
//!
//! We treat `cloud_roles.code = 'super_admin'` as having full access across
//! all organizations. This is enforced at the API layer by checking the
//! caller's user id against this helper.

use sqlx::MySqlPool;

/// Returns true if the given user has the `super_admin` role.
pub async fn is_super_admin(pool: &MySqlPool, user_id: &str) -> Result<bool, sqlx::Error> {
    let (exists,): (i64,) = sqlx::query_as(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM org_memberships om
          JOIN cloud_roles r ON r.id = om.role_id
          WHERE om.user_id = ? AND r.code = 'super_admin'
        ) AS has_role
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(exists != 0)
}

