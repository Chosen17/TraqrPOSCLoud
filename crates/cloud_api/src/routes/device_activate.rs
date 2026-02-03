use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::state::AppState;
use db::{
    create_device, create_device_sync_state, create_device_token, find_activation_key_by_hash,
    increment_activation_key_uses, resolve_store_for_activation,
};
use domain::{ActivateDeviceRequest, ActivateDeviceResponse};

pub fn router(_state: AppState) -> axum::Router<AppState> {
    Router::new().route("/device/activate", post(activate_device))
}

fn hash_activation_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

async fn activate_device(
    State(state): State<AppState>,
    Json(req): Json<ActivateDeviceRequest>,
) -> Result<Json<ActivateDeviceResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let key_hash = hash_activation_key(&req.activation_key);
    let key_row = find_activation_key_by_hash(db, &key_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "invalid or revoked activation key".to_string()))?;

    if let Some(exp) = key_row.expires_at {
        if exp < chrono::Utc::now() {
            return Err((
                StatusCode::UNAUTHORIZED,
                "activation key has expired".to_string(),
            ));
        }
    }
    if let Some(max) = key_row.max_uses {
        if key_row.uses_count >= max {
            return Err((
                StatusCode::UNAUTHORIZED,
                "activation key has reached maximum uses".to_string(),
            ));
        }
    }

    let store_id = resolve_store_for_activation(
        db,
        key_row.org_id,
        &key_row.scope_type,
        key_row.scope_id,
        req.store_hint,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "could not resolve store for activation (provide store_hint if scope is franchise/org)"
                .to_string(),
        )
    })?;

    let device_id = create_device(
        db,
        key_row.org_id,
        store_id,
        None,
        None,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let raw_token = format!("devtok_{}", Uuid::new_v4());
    let token_hash = hash_activation_key(&raw_token);
    create_device_token(db, device_id, &token_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    create_device_sync_state(db, device_id, key_row.org_id, store_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    increment_activation_key_uses(db, key_row.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ActivateDeviceResponse {
        device_id,
        org_id: key_row.org_id,
        store_id,
        device_token: raw_token,
        polling_interval_seconds: 10,
    }))
}
