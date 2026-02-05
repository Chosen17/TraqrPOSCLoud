//! Cloud user profiles: avatar path, phone, job title, bio.

use sqlx::MySqlPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserProfileRow {
    pub user_id: String,
    pub avatar_path: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub bio: Option<String>,
}

pub async fn get_profile(pool: &MySqlPool, user_id: &str) -> Result<Option<UserProfileRow>, sqlx::Error> {
    let row = sqlx::query_as(
        r#"SELECT user_id, avatar_path, phone, job_title, bio FROM cloud_user_profiles WHERE user_id = ?"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_profile(
    pool: &MySqlPool,
    user_id: &str,
    avatar_path: Option<&str>,
    phone: Option<&str>,
    job_title: Option<&str>,
    bio: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO cloud_user_profiles (user_id, avatar_path, phone, job_title, bio)
        VALUES (?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
          avatar_path = COALESCE(VALUES(avatar_path), avatar_path),
          phone = VALUES(phone),
          job_title = VALUES(job_title),
          bio = VALUES(bio)
        "#,
    )
    .bind(user_id)
    .bind(avatar_path)
    .bind(phone)
    .bind(job_title)
    .bind(bio)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_avatar_path(pool: &MySqlPool, user_id: &str, avatar_path: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO cloud_user_profiles (user_id, avatar_path) VALUES (?, ?)
        ON DUPLICATE KEY UPDATE avatar_path = VALUES(avatar_path)
        "#,
    )
    .bind(user_id)
    .bind(avatar_path)
    .execute(pool)
    .await?;
    Ok(())
}
