//! Tenancy: organizations and stores (for admin activation key creation).

use sqlx::MySqlPool;
use uuid::Uuid;

/// Create an organization. Returns id. Slug must be unique.
pub async fn create_organization(
    pool: &MySqlPool,
    name: &str,
    slug: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO organizations (id, name, slug, status)
        VALUES (?, ?, ?, 'active')
        "#,
    )
    .bind(id.to_string())
    .bind(name)
    .bind(slug)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Get organization id by slug, if it exists.
pub async fn get_org_id_by_slug(pool: &MySqlPool, slug: &str) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM organizations WHERE slug = ?")
            .bind(slug)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(s,)| Uuid::parse_str(&s).ok()))
}

/// Create a store under an org. Returns id.
pub async fn create_store(
    pool: &MySqlPool,
    org_id: Uuid,
    name: &str,
    code: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO stores (id, org_id, name, code, status)
        VALUES (?, ?, ?, ?, 'active')
        "#,
    )
    .bind(id.to_string())
    .bind(org_id.to_string())
    .bind(name)
    .bind(code)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Get first store id for an org (for scope_type org when no store_hint).
pub async fn get_first_store_id_for_org(
    pool: &MySqlPool,
    org_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM stores WHERE org_id = ? AND status = 'active' ORDER BY created_at LIMIT 1",
    )
    .bind(org_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(s,)| Uuid::parse_str(&s).ok()))
}
