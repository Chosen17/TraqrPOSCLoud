//! Blogs: owner and manager can create/edit. Public read by slug.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use axum::extract::Multipart;
use serde::Deserialize;

use crate::session::CurrentUser;
use crate::state::AppState;
use db::{
    create_blog, delete_blog, get_blog_by_id, get_blog_by_slug, get_traqr_internal_role, list_blogs,
    slug_from_title, update_blog,
};

fn can_manage_blogs(role: Option<&str>) -> bool {
    matches!(role, Some("sa_owner") | Some("sa_manager"))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/portal/blogs", get(list_blogs_handler))
        .route("/portal/blogs/public", get(list_public_blogs))
        .route("/portal/blogs/by-slug/:slug", get(get_blog_by_slug_handler))
        .route("/portal/blogs/upload-image", post(upload_blog_image))
        .route("/portal/blogs", post(create_blog_handler))
        .route("/portal/blogs/:id", get(get_blog_handler))
        .route("/portal/blogs/:id", post(update_blog_handler))
        .route("/portal/blogs/:id", delete(delete_blog_handler))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct ListBlogsQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct BlogOut {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub body: String,
    pub featured_image_path: Option<String>,
    pub author_id: String,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn blog_to_out(row: &db::BlogRow) -> BlogOut {
    BlogOut {
        id: row.id.clone(),
        title: row.title.clone(),
        slug: row.slug.clone(),
        excerpt: row.excerpt.clone(),
        body: row.body.clone(),
        featured_image_path: row.featured_image_path.clone(),
        author_id: row.author_id.clone(),
        published_at: row.published_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
        created_at: row.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
        updated_at: row.updated_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
    }
}

async fn list_blogs_handler(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<ListBlogsQuery>,
) -> Result<Json<Vec<BlogOut>>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_blogs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can list all blogs"));
    }
    let limit = q.limit.unwrap_or(50).min(100);
    let rows = list_blogs(db, true, limit).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "list failed"))?;
    Ok(Json(rows.iter().map(blog_to_out).collect()))
}

async fn list_public_blogs(
    State(state): State<AppState>,
    Query(q): Query<ListBlogsQuery>,
) -> Result<Json<Vec<BlogOut>>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let limit = q.limit.unwrap_or(20).min(50);
    let rows = list_blogs(db, false, limit).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "list failed"))?;
    Ok(Json(rows.iter().map(blog_to_out).collect()))
}

async fn get_blog_by_slug_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<BlogOut>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let row = get_blog_by_slug(db, &slug).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "lookup failed"))?;
    let row = row.ok_or((StatusCode::NOT_FOUND, "blog not found"))?;
    Ok(Json(blog_to_out(&row)))
}

async fn upload_blog_image(
    State(state): State<AppState>,
    user: CurrentUser,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_blogs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can upload blog images"));
    }
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".to_string());
    let base = std::path::Path::new(&upload_dir);
    let base = if base.is_relative() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join(base)
    } else {
        base.to_path_buf()
    };
    let blog_dir = base.join("blogs");
    let _ = tokio::fs::create_dir_all(&blog_dir).await;

    let mut ext = "jpg".to_string();
    let mut data = None;
    while let Some(field) = multipart.next_field().await.map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart"))? {
        let name = field.name().unwrap_or("");
        if name != "file" && name != "image" {
            continue;
        }
        if let Some(name) = field.file_name() {
            ext = std::path::Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .filter(|e| matches!(*e, "jpg" | "jpeg" | "png" | "gif" | "webp"))
                .unwrap_or("jpg")
                .to_string();
        }
        let bytes = field.bytes().await.map_err(|_| (StatusCode::BAD_REQUEST, "failed to read file"))?.to_vec();
        if bytes.len() > 5 * 1024 * 1024 {
            return Err((StatusCode::PAYLOAD_TOO_LARGE, "file too large (max 5MB)"));
        }
        data = Some(bytes);
        break;
    }
    let data = data.ok_or((StatusCode::BAD_REQUEST, "missing file field"))?;

    let filename = format!("{}.{}", uuid::Uuid::new_v4(), ext.as_str());
    let path = blog_dir.join(&filename);
    tokio::fs::write(&path, &data).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to save file"))?;

    let relative_path = format!("blogs/{}", filename);
    Ok(Json(serde_json::json!({ "url": format!("/uploads/{}", relative_path), "path": relative_path })))
}

#[derive(Debug, Deserialize)]
pub struct CreateBlogBody {
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub body: String,
    pub featured_image_path: Option<String>,
    pub publish: Option<bool>,
}

async fn create_blog_handler(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateBlogBody>,
) -> Result<(StatusCode, Json<BlogOut>), (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_blogs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can create blogs"));
    }
    let slug = body.slug.unwrap_or_else(|| slug_from_title(&body.title));
    let published_at = if body.publish == Some(true) {
        Some(chrono::Utc::now().naive_utc())
    } else {
        None
    };
    let id = create_blog(
        db,
        &body.title,
        &slug,
        body.excerpt.as_deref(),
        &body.body,
        body.featured_image_path.as_deref(),
        &user.0,
        published_at,
    )
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "create failed"))?;
    let row = get_blog_by_id(db, &id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "fetch failed"))?.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "blog not found"))?;
    Ok((StatusCode::CREATED, Json(blog_to_out(&row))))
}

async fn get_blog_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<BlogOut>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let row = get_blog_by_id(db, &id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "lookup failed"))?;
    let row = row.ok_or((StatusCode::NOT_FOUND, "blog not found"))?;
    Ok(Json(blog_to_out(&row)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBlogBody {
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub body: String,
    pub featured_image_path: Option<String>,
    /// RFC3339 to publish, or omit to keep current
    pub published_at: Option<String>,
}

async fn update_blog_handler(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateBlogBody>,
) -> Result<Json<BlogOut>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_blogs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can edit blogs"));
    }
    let slug = body.slug.unwrap_or_else(|| slug_from_title(&body.title));
    let published_at = if let Some(ref s) = body.published_at {
        chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.naive_utc())
    } else {
        get_blog_by_id(db, &id).await.ok().flatten().and_then(|r| r.published_at)
    };
    update_blog(db, &id, &body.title, &slug, body.excerpt.as_deref(), &body.body, body.featured_image_path.as_deref(), published_at)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "update failed"))?;
    let row = get_blog_by_id(db, &id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "fetch failed"))?.ok_or((StatusCode::NOT_FOUND, "blog not found"))?;
    Ok(Json(blog_to_out(&row)))
}

async fn delete_blog_handler(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_blogs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can delete blogs"));
    }
    delete_blog(db, &id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "delete failed"))?;
    Ok(StatusCode::NO_CONTENT)
}
