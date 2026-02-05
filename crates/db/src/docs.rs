//! Docs: guides and setup/usage. Section = sidebar group; sort_order for ordering.

use sqlx::MySqlPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DocRow {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub section: String,
    pub sort_order: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

pub async fn list_docs(pool: &MySqlPool) -> Result<Vec<DocRow>, sqlx::Error> {
    let rows = sqlx::query_as(
        r#"
        SELECT id, title, slug, body, section, sort_order, created_at, updated_at
        FROM docs ORDER BY section ASC, sort_order ASC, title ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_doc_by_slug(pool: &MySqlPool, slug: &str) -> Result<Option<DocRow>, sqlx::Error> {
    let row = sqlx::query_as(
        r#"
        SELECT id, title, slug, body, section, sort_order, created_at, updated_at
        FROM docs WHERE slug = ?
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_doc_by_id(pool: &MySqlPool, id: &str) -> Result<Option<DocRow>, sqlx::Error> {
    let row = sqlx::query_as(
        r#"
        SELECT id, title, slug, body, section, sort_order, created_at, updated_at
        FROM docs WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn create_doc(
    pool: &MySqlPool,
    title: &str,
    slug: &str,
    body: &str,
    section: &str,
    sort_order: i32,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO docs (id, title, slug, body, section, sort_order)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(title)
    .bind(slug)
    .bind(body)
    .bind(section)
    .bind(sort_order)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn update_doc(
    pool: &MySqlPool,
    id: &str,
    title: &str,
    slug: &str,
    body: &str,
    section: &str,
    sort_order: i32,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        r#"
        UPDATE docs SET title = ?, slug = ?, body = ?, section = ?, sort_order = ? WHERE id = ?
        "#,
    )
    .bind(title)
    .bind(slug)
    .bind(body)
    .bind(section)
    .bind(sort_order)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn delete_doc(pool: &MySqlPool, id: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("DELETE FROM docs WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

