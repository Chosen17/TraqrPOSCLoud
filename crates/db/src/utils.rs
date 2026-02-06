//! Shared helpers (e.g. slug generation, auth/tenant checks).

use sqlx::MySqlPool;
use uuid::Uuid;

pub fn slug_from_title(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| match c {
            'A'..='Z' => char::from(c as u8 + 32),
            'a'..='z' | '0'..='9' => c,
            ' ' | '-' | '_' => '-',
            _ => '\0',
        })
        .filter(|c| *c != '\0')
        .collect();
    s.split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Returns true if the user can access the given organization.
/// Rules:
/// - super_admin can access all orgs
/// - otherwise user must have an active org_membership for that org.
pub async fn user_can_access_org(
    pool: &MySqlPool,
    user_id: &str,
    org_id: Uuid,
) -> Result<bool, sqlx::Error> {
    // super_admin check
    let (is_super_admin,): (i64,) = sqlx::query_as(
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
    if is_super_admin != 0 {
        return Ok(true);
    }

    let (has_membership,): (i64,) = sqlx::query_as(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM org_memberships om
          WHERE om.user_id = ? AND om.org_id = ? AND om.status = 'active'
        ) AS has_membership
        "#,
    )
    .bind(user_id)
    .bind(org_id.to_string())
    .fetch_one(pool)
    .await?;

    Ok(has_membership != 0)
}

/// Returns true if the user can access the given store.
/// Rules:
/// - super_admin can access all stores
/// - otherwise user must either:
///   - have an active store_memberships row for that store, OR
///   - have an active org_memberships row for the store's org.
pub async fn user_can_access_store(
    pool: &MySqlPool,
    user_id: &str,
    store_id: Uuid,
) -> Result<bool, sqlx::Error> {
    // super_admin check
    let (is_super_admin,): (i64,) = sqlx::query_as(
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
    if is_super_admin != 0 {
        return Ok(true);
    }

    // Direct store membership
    let (has_store_membership,): (i64,) = sqlx::query_as(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM store_memberships sm
          WHERE sm.user_id = ? AND sm.store_id = ? AND sm.status = 'active'
        ) AS has_store_membership
        "#,
    )
    .bind(user_id)
    .bind(store_id.to_string())
    .fetch_one(pool)
    .await?;
    if has_store_membership != 0 {
        return Ok(true);
    }

    // Fallback: org membership for the store's org.
    let org_row: Option<(String,)> =
        sqlx::query_as("SELECT org_id FROM stores WHERE id = ?")
            .bind(store_id.to_string())
            .fetch_optional(pool)
            .await?;
    let Some((org_id,)) = org_row else {
        return Ok(false);
    };

    let (has_org_membership,): (i64,) = sqlx::query_as(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM org_memberships om
          WHERE om.user_id = ? AND om.org_id = ? AND om.status = 'active'
        ) AS has_org_membership
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_one(pool)
    .await?;

    Ok(has_org_membership != 0)
}
