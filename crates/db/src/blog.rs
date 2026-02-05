//! Blogs: CRUD for owner/manager. SEO-friendly slug.

use sqlx::MySqlPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BlogRow {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub body: String,
    pub featured_image_path: Option<String>,
    pub author_id: String,
    pub published_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

pub async fn list_blogs(
    pool: &MySqlPool,
    include_drafts: bool,
    limit: i64,
) -> Result<Vec<BlogRow>, sqlx::Error> {
    let rows = if include_drafts {
        sqlx::query_as(
            r#"
            SELECT id, title, slug, excerpt, body, featured_image_path, author_id, published_at, created_at, updated_at
            FROM blogs ORDER BY COALESCE(published_at, updated_at) DESC LIMIT ?
            "#,
        )
    } else {
        sqlx::query_as(
            r#"
            SELECT id, title, slug, excerpt, body, featured_image_path, author_id, published_at, created_at, updated_at
            FROM blogs WHERE published_at IS NOT NULL ORDER BY published_at DESC LIMIT ?
            "#,
        )
    }
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_blog_by_id(pool: &MySqlPool, id: &str) -> Result<Option<BlogRow>, sqlx::Error> {
    let row = sqlx::query_as(
        r#"
        SELECT id, title, slug, excerpt, body, featured_image_path, author_id, published_at, created_at, updated_at
        FROM blogs WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_blog_by_slug(pool: &MySqlPool, slug: &str) -> Result<Option<BlogRow>, sqlx::Error> {
    let row = sqlx::query_as(
        r#"
        SELECT id, title, slug, excerpt, body, featured_image_path, author_id, published_at, created_at, updated_at
        FROM blogs WHERE slug = ? AND published_at IS NOT NULL
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn create_blog(
    pool: &MySqlPool,
    title: &str,
    slug: &str,
    excerpt: Option<&str>,
    body: &str,
    featured_image_path: Option<&str>,
    author_id: &str,
    published_at: Option<chrono::NaiveDateTime>,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO blogs (id, title, slug, excerpt, body, featured_image_path, author_id, published_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(title)
    .bind(slug)
    .bind(excerpt)
    .bind(body)
    .bind(featured_image_path)
    .bind(author_id)
    .bind(published_at)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn update_blog(
    pool: &MySqlPool,
    id: &str,
    title: &str,
    slug: &str,
    excerpt: Option<&str>,
    body: &str,
    featured_image_path: Option<&str>,
    published_at: Option<chrono::NaiveDateTime>,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        r#"
        UPDATE blogs SET title = ?, slug = ?, excerpt = ?, body = ?, featured_image_path = ?, published_at = ? WHERE id = ?
        "#,
    )
    .bind(title)
    .bind(slug)
    .bind(excerpt)
    .bind(body)
    .bind(featured_image_path)
    .bind(published_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn delete_blog(pool: &MySqlPool, id: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("DELETE FROM blogs WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

