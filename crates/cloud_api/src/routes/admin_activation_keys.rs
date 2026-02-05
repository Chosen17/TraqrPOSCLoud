//! Admin: create activation keys. Protected by ADMIN_API_KEY. Raw key returned once.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use sha2::{Digest, Sha256};

use crate::state::AppState;
use db::{
    create_activation_key, create_organization, create_store, get_org_id_by_slug,
};
use domain::{CreateActivationKeyRequest, CreateActivationKeyResponse};

pub fn router(_state: AppState) -> axum::Router<AppState> {
    Router::new()
        .route("/admin/activation-keys", post(create_activation_key_handler))
}

fn hash_activation_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Generate a short activation key (easy to type; 64-bit entropy, hashed before storage).
fn generate_activation_key() -> String {
    let u = uuid::Uuid::new_v4();
    let b = u.as_bytes();
    format!(
        "traqr-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]
    )
}

async fn create_activation_key_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateActivationKeyRequest>,
) -> Result<Json<CreateActivationKeyResponse>, (StatusCode, String)> {
    if req.scope_type != "store" && req.scope_type != "franchise" && req.scope_type != "org" {
        return Err((
            StatusCode::BAD_REQUEST,
            "scope_type must be 'store', 'franchise', or 'org'".to_string(),
        ));
    }

    // Optional admin key check
    if let Ok(expect) = std::env::var("ADMIN_API_KEY") {
        if !expect.is_empty() {
            let provided = headers
                .get("X-Admin-Key")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if provided != expect {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "missing or invalid X-Admin-Key".to_string(),
                ));
            }
        }
    }

    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;

    let (org_id, store_id, scope_id) = if let (Some(oid), Some(sid)) = (req.org_id, req.store_id) {
        // Use existing org/store; scope_id for store scope is store_id
        let scope_id = req
            .scope_id
            .or_else(|| (req.scope_type == "store").then_some(sid));
        (oid, sid, scope_id)
    } else if let (Some(org_name), Some(org_slug), Some(store_name)) =
        (req.org_name.as_deref(), req.org_slug.as_deref(), req.store_name.as_deref())
    {
        // Create org and store
        let org_id = match get_org_id_by_slug(db, org_slug)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        {
            Some(id) => id,
            None => create_organization(db, org_name, org_slug)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
        };
        let store_id = create_store(db, org_id, store_name, None)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let scope_id = req.scope_id.or(Some(store_id));
        (org_id, store_id, scope_id)
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "provide either (org_id + store_id) or (org_name + org_slug + store_name)".to_string(),
        ));
    };

    if req.scope_type == "store" && scope_id != Some(store_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "scope_type 'store' requires scope_id equal to store_id".to_string(),
        ));
    }

    if req.scope_type == "org" && scope_id.is_some() {
        // org scope typically has scope_id = null
    }

    let raw_key = generate_activation_key();
    let key_hash = hash_activation_key(&raw_key);

    let expires_at = req
        .expires_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let key_id = create_activation_key(
        db,
        org_id,
        &req.scope_type,
        scope_id,
        &key_hash,
        req.max_uses,
        expires_at,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(CreateActivationKeyResponse {
        activation_key: raw_key,
        key_id,
        org_id,
        store_id,
        scope_type: req.scope_type,
        scope_id,
        max_uses: req.max_uses,
        expires_at: req.expires_at,
    }))
}
