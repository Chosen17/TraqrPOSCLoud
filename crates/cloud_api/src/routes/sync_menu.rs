//! GET /api/sync/menu: device pulls current menu for its store (or copy_from_store_id for new store).
//! POST /api/sync/upload-item-image: device uploads a menu item image; returns path to use in menu_item_created / menu_item_image events.

use axum::{
    extract::{Multipart, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::state::AppState;
use db::{get_store_menu_for_sync, has_active_entitlement, validate_device_token, SyncMenuCategory, SyncMenuItem};

fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Deserialize)]
pub struct SyncMenuQuery {
    /// Same-org store id to copy menu from (e.g. new store gets menu from existing store).
    pub copy_from_store_id: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct SyncMenuResponse {
    pub categories: Vec<SyncMenuCategory>,
    pub items: Vec<SyncMenuItem>,
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/sync/menu", get(get_menu))
        .route("/sync/upload-item-image", post(upload_item_image))
}

async fn get_menu(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SyncMenuQuery>,
) -> Result<Json<SyncMenuResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;

    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "missing or invalid Authorization".to_string()))?;
    let token_hash = hash_token(token);
    let identity = validate_device_token(db, &token_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "invalid or revoked device token".to_string()))?;

    let cloud_sync_ok = has_active_entitlement(db, identity.org_id, "cloud_sync")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !cloud_sync_ok {
        return Err((
            StatusCode::FORBIDDEN,
            "Cloud sync not enabled for this organization".to_string(),
        ));
    }

    let store_id = if let Some(ref copy_id) = q.copy_from_store_id {
        let copy_uuid = uuid::Uuid::parse_str(copy_id)
            .map_err(|_| (StatusCode::BAD_REQUEST, "invalid copy_from_store_id".to_string()))?;
        let exists: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM stores WHERE id = ? AND org_id = ?",
        )
        .bind(copy_uuid.to_string())
        .bind(identity.org_id.to_string())
        .fetch_optional(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if exists.is_none() {
            return Err((
                StatusCode::FORBIDDEN,
                "store not found or not in your organization".to_string(),
            ));
        }
        copy_uuid
    } else {
        identity.store_id
    };

    let menu = get_store_menu_for_sync(db, store_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (categories, items) = menu.unwrap_or((Vec::new(), Vec::new()));

    Ok(Json(SyncMenuResponse { categories, items }))
}

/// POST /api/sync/upload-item-image
/// Device uploads an image for a menu item. Returns path/url to send in menu_item_created or menu_item_image event.
async fn upload_item_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;

    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "missing or invalid Authorization".to_string()))?;
    let token_hash = hash_token(token);
    let identity = validate_device_token(db, &token_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "invalid or revoked device token".to_string()))?;

    let cloud_sync_ok = has_active_entitlement(db, identity.org_id, "cloud_sync")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !cloud_sync_ok {
        return Err((
            StatusCode::FORBIDDEN,
            "Cloud sync not enabled for this organization".to_string(),
        ));
    }

    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".to_string());
    let base = std::path::Path::new(&upload_dir);
    let base = if base.is_relative() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join(base)
    } else {
        base.to_path_buf()
    };
    let menu_dir = base.join("menu");
    let _ = tokio::fs::create_dir_all(&menu_dir).await;

    let mut ext = "jpg".to_string();
    let mut data = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart".to_string()))?
    {
        // Accept field named "file", "image", or any part that has a file name (common alternatives: "photo", "picture")
        let name = field.name().unwrap_or("");
        let has_file_name = field.file_name().is_some();
        if name != "file" && name != "image" && !has_file_name {
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
        let bytes = field
            .bytes()
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, "failed to read file".to_string()))?
            .to_vec();
        if bytes.len() > 5 * 1024 * 1024 {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "file too large (max 5MB)".to_string(),
            ));
        }
        data = Some(bytes);
        break;
    }
    let data = data.ok_or((StatusCode::BAD_REQUEST, "missing file field (file or image)".to_string()))?;

    let filename = format!("{}.{}", uuid::Uuid::new_v4(), ext.as_str());
    let path = menu_dir.join(&filename);
    tokio::fs::write(&path, &data)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to save file".to_string()))?;

    let relative_path = format!("menu/{}", filename);
    Ok(Json(serde_json::json!({
        "url": format!("/uploads/{}", relative_path),
        "path": relative_path
    })))
}
