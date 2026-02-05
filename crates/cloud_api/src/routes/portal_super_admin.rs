use axum::{extract::State, http::StatusCode, routing::get, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::state::AppState;
use db::{
    create_activation_key, create_organization, create_store, get_org_id_by_slug, grant_entitlement,
};

#[derive(Debug, Serialize)]
pub struct OrgSummary {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub created_at: String,
    pub store_count: i64,
    pub device_count: i64,
    pub cloud_sync_active: bool,
}

#[derive(Debug, Serialize)]
pub struct SuperAdminOrgList {
    pub organizations: Vec<OrgSummary>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCustomerRequest {
    pub org_name: String,
    pub org_slug: String,
    pub store_name: String,
    #[serde(default)]
    pub grant_cloud_sync: bool,
}

#[derive(Debug, Serialize)]
pub struct CreateCustomerResponse {
    pub activation_key: String,
    pub org_id: Uuid,
    pub store_id: Uuid,
    pub org_slug: String,
    pub cloud_sync_granted: bool,
}

fn hash_activation_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Short key format so staff can type it into the POS (64-bit entropy, hashed in DB).
fn generate_activation_key() -> String {
    let u = Uuid::new_v4();
    let b = u.as_bytes();
    format!(
        "traqr-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]
    )
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/portal/super/orgs", get(list_orgs_for_super_admin))
        .route("/portal/super/create-customer", post(create_customer))
}

async fn list_orgs_for_super_admin(
    State(state): State<AppState>,
) -> Result<Json<SuperAdminOrgList>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;

    // NOTE: v1 does not actually authenticate the caller as super_admin; this
    // is wired for internal use only. When auth_sessions are implemented we
    // can enforce a stricter check here using db::is_super_admin.

    let rows = sqlx::query(
        r#"
        SELECT
          o.id,
          o.name,
          o.slug,
          o.created_at,
          (SELECT COUNT(*) FROM stores s WHERE s.org_id = o.id) AS store_count,
          (SELECT COUNT(*) FROM devices d WHERE d.org_id = o.id) AS device_count,
          EXISTS(
            SELECT 1
            FROM org_entitlements oe
            JOIN plans p ON p.id = oe.plan_id
            WHERE oe.org_id = o.id
              AND p.code = 'cloud_sync'
              AND (oe.valid_until IS NULL OR oe.valid_until > CURRENT_TIMESTAMP(3))
          ) AS cloud_sync_active
        FROM organizations o
        ORDER BY o.created_at DESC
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let organizations = rows
        .into_iter()
        .map(|row| OrgSummary {
            id: row.get::<String, _>("id"),
            name: row.get::<String, _>("name"),
            slug: row.get::<Option<String>, _>("slug"),
            created_at: row
                .get::<chrono::NaiveDateTime, _>("created_at")
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            store_count: row.get::<i64, _>("store_count"),
            device_count: row.get::<i64, _>("device_count"),
            cloud_sync_active: row.get::<bool, _>("cloud_sync_active"),
        })
        .collect();

    Ok(Json(SuperAdminOrgList { organizations }))
}

async fn create_customer(
    State(state): State<AppState>,
    Json(req): Json<CreateCustomerRequest>,
) -> Result<Json<CreateCustomerResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;

    let org_id = match get_org_id_by_slug(db, req.org_slug.trim())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        Some(id) => id,
        None => create_organization(db, req.org_name.trim(), req.org_slug.trim())
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    };

    let store_id = create_store(db, org_id, req.store_name.trim(), None)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let raw_key = generate_activation_key();
    let key_hash = hash_activation_key(&raw_key);
    create_activation_key(db, org_id, "store", Some(store_id), &key_hash, None, None)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if req.grant_cloud_sync {
        grant_entitlement(db, org_id, "cloud_sync")
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(Json(CreateCustomerResponse {
        activation_key: raw_key,
        org_id,
        store_id,
        org_slug: req.org_slug.trim().to_string(),
        cloud_sync_granted: req.grant_cloud_sync,
    }))
}

fn internal<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

