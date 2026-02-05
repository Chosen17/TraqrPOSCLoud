//! Current user profile: get/update, avatar upload.

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    routing::{patch, post},
    Json, Router,
};
use serde::Deserialize;

use crate::session::CurrentUser;
use crate::state::AppState;
use db::{get_profile, set_avatar_path, upsert_profile};

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/portal/me", patch(update_profile))
        .route("/portal/me/avatar", post(upload_avatar))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileBody {
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub bio: Option<String>,
}

async fn update_profile(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<UpdateProfileBody>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let existing = get_profile(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "profile lookup failed"))?;
    upsert_profile(
        db,
        &user.0,
        existing.as_ref().and_then(|p| p.avatar_path.as_deref()),
        body.phone.as_deref(),
        body.job_title.as_deref(),
        body.bio.as_deref(),
    )
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "profile update failed"))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn upload_avatar(
    State(state): State<AppState>,
    user: CurrentUser,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".to_string());
    let base = std::path::Path::new(&upload_dir);
    let base = if base.is_relative() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join(base)
    } else {
        base.to_path_buf()
    };
    let avatar_dir = base.join("avatars");
    let _ = tokio::fs::create_dir_all(&avatar_dir).await;

    let mut ext = "jpg".to_string();
    let mut data = None;
    while let Some(field) = multipart.next_field().await.map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart"))? {
        if field.name() != Some("file") {
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

    let filename = format!("{}.{}", user.0, ext.as_str());
    let path = avatar_dir.join(&filename);
    tokio::fs::write(&path, &data).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to save file"))?;

    let relative_path = format!("avatars/{}", filename);
    set_avatar_path(db, &user.0, &relative_path).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to update profile"))?;

    Ok(Json(serde_json::json!({ "avatar_path": relative_path })))
}
