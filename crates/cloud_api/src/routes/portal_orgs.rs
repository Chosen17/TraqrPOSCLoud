use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::session::CurrentUser;
use crate::state::AppState;
use db::{reactivate_cloud_sync, suspend_cloud_sync};

#[derive(Debug, Serialize)]
pub struct OrgSummary {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub created_at: String,
    pub store_count: i64,
    pub cloud_sync_active: bool,
}

#[derive(Debug, Serialize)]
pub struct OrgListResponse {
    pub organizations: Vec<OrgSummary>,
}

#[derive(Debug, Serialize)]
pub struct OrgEntitlement {
    pub plan_code: String,
    pub plan_name: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub cloud_sync_add_on: bool,
}

#[derive(Debug, Serialize)]
pub struct StoreSummary {
    pub id: String,
    pub name: String,
    pub timezone: Option<String>,
    /// Canonical (primary) device id for this store, if set.
    pub canonical_device_id: Option<String>,
    pub device_count: i64,
    pub last_event_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrgDetailResponse {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub created_at: String,
    pub entitlements: Vec<OrgEntitlement>,
    pub stores: Vec<StoreSummary>,
}

#[derive(Debug, Serialize)]
pub struct DeviceRow {
    pub id: String,
    pub local_device_id: Option<String>,
    pub device_label: Option<String>,
    pub device_name: Option<String>,
    pub is_primary: bool,
    pub hardware_fingerprint: Option<String>,
    pub status: String,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StoreDevicesResponse {
    pub store_id: String,
    pub devices: Vec<DeviceRow>,
}

#[derive(Debug, Serialize)]
pub struct ActivationKeyRow {
    pub id: String,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub max_uses: Option<i32>,
    pub uses_count: i32,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StoreActivationKeysResponse {
    pub store_id: String,
    pub keys: Vec<ActivationKeyRow>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCloudSyncEntitlementRequest {
    /// Optional: end date/time in RFC3339. If omitted, entitlement is open-ended.
    pub valid_until: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OrgDetailParams {
    pub org_id: String,
}

#[derive(Debug, Deserialize)]
pub struct StoreParams {
    pub store_id: String,
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/portal/orgs", get(list_orgs))
        .route("/portal/orgs/:org_id", get(get_org_detail))
        .route(
            "/portal/orgs/:org_id/entitlements/cloud_sync",
            post(create_cloud_sync_entitlement),
        )
        .route("/portal/orgs/:org_id/suspend", post(suspend_org))
        .route("/portal/orgs/:org_id/reactivate", post(reactivate_org))
        .route("/portal/stores/:store_id/devices", get(get_store_devices))
        .route(
            "/portal/stores/:store_id/activation-keys",
            get(get_store_activation_keys),
        )
}

async fn list_orgs(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<OrgListResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;

    // We consider cloud_sync_active true if there is any active org_entitlement
    // for plan code 'cloud_sync'.
    // Only return orgs the current user can access (membership or super_admin).
    let rows = sqlx::query(
        r#"
        SELECT
          o.id,
          o.name,
          o.slug,
          o.created_at,
          (SELECT COUNT(*) FROM stores s WHERE s.org_id = o.id) AS store_count,
          EXISTS(
            SELECT 1
            FROM org_entitlements oe
            JOIN plans p ON p.id = oe.plan_id
            WHERE oe.org_id = o.id
              AND p.code = 'cloud_sync'
              AND (oe.valid_until IS NULL OR oe.valid_until > CURRENT_TIMESTAMP(3))
          ) AS cloud_sync_active
        FROM organizations o
        WHERE EXISTS (
          SELECT 1
          FROM org_memberships om
          WHERE om.org_id = o.id AND om.user_id = ? AND om.status = 'active'
        )
        OR EXISTS (
          SELECT 1
          FROM org_memberships om2
          JOIN cloud_roles r2 ON r2.id = om2.role_id
          WHERE om2.user_id = ? AND r2.code = 'super_admin' AND om2.status = 'active'
        )
        ORDER BY o.created_at DESC
        "#,
    )
    .bind(&user.0)
    .bind(&user.0)
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
            cloud_sync_active: row.get::<bool, _>("cloud_sync_active"),
        })
        .collect();

    Ok(Json(OrgListResponse { organizations }))
}

/// Fetch org detail by id (no auth). Used by get_org_detail and super-admin org detail.
pub async fn fetch_org_detail_by_id(
    db: &sqlx::MySqlPool,
    org_uuid: Uuid,
) -> Result<OrgDetailResponse, (StatusCode, String)> {
    let org_row = sqlx::query(
        r#"
        SELECT id, name, slug, created_at
        FROM organizations
        WHERE id = ?
        "#,
    )
    .bind(org_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(internal)?;

    let Some(org_row) = org_row else {
        return Err((StatusCode::NOT_FOUND, "organization not found".to_string()));
    };

    let ent_rows = sqlx::query(
        r#"
        SELECT
          p.code AS plan_code,
          p.name AS plan_name,
          oe.valid_from,
          oe.valid_until,
          oe.cloud_sync_add_on
        FROM org_entitlements oe
        JOIN plans p ON p.id = oe.plan_id
        WHERE oe.org_id = ?
        ORDER BY oe.created_at DESC
        "#,
    )
    .bind(org_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let entitlements = ent_rows
        .into_iter()
        .map(|row| OrgEntitlement {
            plan_code: row.get::<String, _>("plan_code"),
            plan_name: row.get::<String, _>("plan_name"),
            valid_from: row
                .get::<chrono::NaiveDateTime, _>("valid_from")
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            valid_until: row
                .get::<Option<chrono::NaiveDateTime>, _>("valid_until")
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
            cloud_sync_add_on: row.get::<bool, _>("cloud_sync_add_on"),
        })
        .collect();

    let store_rows = sqlx::query(
        r#"
        SELECT
          s.id,
          s.name,
          s.timezone,
          s.canonical_device_id,
          COALESCE(d.device_count, 0) AS device_count,
          es.last_event_at
        FROM stores s
        LEFT JOIN (
          SELECT store_id, COUNT(*) AS device_count
          FROM devices
          GROUP BY store_id
        ) d ON d.store_id = s.id
        LEFT JOIN (
          SELECT
            store_id,
            MAX(received_at) AS last_event_at
          FROM device_event_log
          GROUP BY store_id
        ) es ON es.store_id = s.id
        WHERE s.org_id = ?
        ORDER BY s.name
        "#,
    )
    .bind(org_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let stores = store_rows
        .into_iter()
        .map(|row| StoreSummary {
            id: row.get::<String, _>("id"),
            name: row.get::<String, _>("name"),
            timezone: row.get::<Option<String>, _>("timezone"),
            canonical_device_id: row.get::<Option<String>, _>("canonical_device_id"),
            device_count: row.get::<i64, _>("device_count"),
            last_event_at: row
                .get::<Option<chrono::NaiveDateTime>, _>("last_event_at")
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
        })
        .collect();

    Ok(OrgDetailResponse {
        id: org_row.get::<String, _>("id"),
        name: org_row.get::<String, _>("name"),
        slug: org_row.get::<Option<String>, _>("slug"),
        created_at: org_row
            .get::<chrono::NaiveDateTime, _>("created_at")
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
        entitlements,
        stores,
    })
}

async fn get_org_detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(OrgDetailParams { org_id }): Path<OrgDetailParams>,
) -> Result<Json<OrgDetailResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let org_uuid = Uuid::parse_str(&org_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid org_id".to_string()))?;

    let allowed = db::user_can_access_org(db, &user.0, org_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "organization not in your account".to_string()));
    }

    let data = fetch_org_detail_by_id(db, org_uuid).await?;
    Ok(Json(data))
}

async fn get_store_devices(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StoreParams { store_id }): Path<StoreParams>,
) -> Result<Json<StoreDevicesResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let rows = sqlx::query(
        r#"
        SELECT
          d.id,
          d.device_label,
          d.device_name,
          d.is_primary,
          d.hardware_fingerprint,
          d.status,
          ss.updated_at AS last_seen_at
        FROM devices d
        LEFT JOIN device_sync_state ss ON ss.device_id = d.id
        WHERE d.store_id = ?
        ORDER BY d.is_primary DESC, d.created_at DESC
        "#,
    )
    .bind(store_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let devices = rows
        .into_iter()
        .map(|row| DeviceRow {
            id: row.get::<String, _>("id"),
            local_device_id: None,
            device_label: row.get::<Option<String>, _>("device_label"),
            device_name: row.get::<Option<String>, _>("device_name"),
            is_primary: row.get::<i8, _>("is_primary") != 0,
            hardware_fingerprint: row.get::<Option<String>, _>("hardware_fingerprint"),
            status: row.get::<String, _>("status"),
            last_seen_at: row
                .get::<Option<chrono::NaiveDateTime>, _>("last_seen_at")
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
        })
        .collect();

    Ok(Json(StoreDevicesResponse {
        store_id,
        devices,
    }))
}

async fn get_store_activation_keys(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StoreParams { store_id }): Path<StoreParams>,
) -> Result<Json<StoreActivationKeysResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let rows = sqlx::query(
        r#"
        SELECT
          id,
          scope_type,
          scope_id,
          max_uses,
          uses_count,
          expires_at,
          revoked_at
        FROM device_activation_keys
        WHERE (scope_type = 'store' AND scope_id = ?)
           OR (scope_type = 'org' AND org_id = (SELECT org_id FROM stores WHERE id = ? LIMIT 1))
        ORDER BY created_at DESC
        "#,
    )
    .bind(store_uuid.to_string())
    .bind(store_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let keys = rows
        .into_iter()
        .map(|row| ActivationKeyRow {
            id: row.get::<String, _>("id"),
            scope_type: row.get::<String, _>("scope_type"),
            scope_id: row.get::<Option<String>, _>("scope_id"),
            max_uses: row.get::<Option<i32>, _>("max_uses"),
            uses_count: row.get::<i32, _>("uses_count"),
            expires_at: row
                .get::<Option<chrono::NaiveDateTime>, _>("expires_at")
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
            revoked_at: row
                .get::<Option<chrono::NaiveDateTime>, _>("revoked_at")
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
        })
        .collect();

    Ok(Json(StoreActivationKeysResponse { store_id, keys }))
}

/// Suspend the customer's account: Cloud Sync stops (no sync until reactivated).
async fn suspend_org(
    State(state): State<AppState>,
    Path(OrgDetailParams { org_id }): Path<OrgDetailParams>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let org_uuid = Uuid::parse_str(&org_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid org_id".to_string()))?;
    let updated = suspend_cloud_sync(db, org_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !updated {
        return Err((
            StatusCode::NOT_FOUND,
            "organization has no Cloud Sync entitlement to suspend".to_string(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Reactivate the customer's account: Cloud Sync works again.
async fn reactivate_org(
    State(state): State<AppState>,
    Path(OrgDetailParams { org_id }): Path<OrgDetailParams>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let org_uuid = Uuid::parse_str(&org_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid org_id".to_string()))?;
    reactivate_cloud_sync(db, org_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Minimal internal-use endpoint to grant a Cloud Sync entitlement for an org.
/// Creates (or updates) an org_entitlements row linked to plan 'cloud_sync'.
async fn create_cloud_sync_entitlement(
    State(state): State<AppState>,
    Path(OrgDetailParams { org_id }): Path<OrgDetailParams>,
    Json(body): Json<CreateCloudSyncEntitlementRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let org_uuid = Uuid::parse_str(&org_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid org_id".to_string()))?;

    // Look up plan id for code 'cloud_sync'.
    let plan_row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM plans WHERE code = 'cloud_sync'")
            .fetch_optional(db)
            .await
            .map_err(internal)?;
    let Some((plan_id,)) = plan_row else {
        return Err((
            StatusCode::BAD_REQUEST,
            "cloud_sync plan not found; run migrations".to_string(),
        ));
    };

    let valid_until = body
        .valid_until
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    // Upsert org_entitlements for this org/plan.
    sqlx::query(
        r#"
        INSERT INTO org_entitlements (org_id, plan_id, cloud_sync_add_on, device_limit, valid_from, valid_until)
        VALUES (?, ?, 1, NULL, CURRENT_TIMESTAMP(3), ?)
        ON DUPLICATE KEY UPDATE
          cloud_sync_add_on = VALUES(cloud_sync_add_on),
          valid_until = VALUES(valid_until)
        "#,
    )
    .bind(org_uuid.to_string())
    .bind(plan_id)
    .bind(valid_until)
    .execute(db)
    .await
    .map_err(internal)?;

    Ok(StatusCode::NO_CONTENT)
}

fn internal<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

