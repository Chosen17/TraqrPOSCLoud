//! Device activation: lookup activation key, create device, issue token.

use sqlx::MySqlPool;
use uuid::Uuid;

/// Activation key row (lookup by key_hash). UUID columns decoded as String from MySQL CHAR(36).
#[derive(Debug, sqlx::FromRow)]
pub struct ActivationKeyRow {
    pub id: String,
    pub org_id: String,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub is_multi_use: bool,
    pub max_uses: Option<i32>,
    pub uses_count: i32,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Find activation key by SHA-256 hash of the key (key_hash).
/// Caller hashes the raw activation_key before calling.
pub async fn find_activation_key_by_hash(
    pool: &MySqlPool,
    key_hash: &str,
) -> Result<Option<ActivationKeyRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, org_id, scope_type, scope_id, is_multi_use, max_uses, uses_count, expires_at, revoked_at
        FROM device_activation_keys
        WHERE key_hash = ? AND revoked_at IS NULL
        "#,
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await
}

/// Resolve store_id for activation: scope_type store -> scope_id; franchise/org -> use store_hint or first store.
pub async fn resolve_store_for_activation(
    pool: &MySqlPool,
    org_id: Uuid,
    scope_type: &str,
    scope_id: Option<Uuid>,
    store_hint: Option<Uuid>,
) -> Result<Option<Uuid>, sqlx::Error> {
    match (scope_type, scope_id, store_hint) {
        ("store", Some(sid), _) => {
            let exists: (i64,) = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM stores WHERE id = ? AND org_id = ?) AS ex",
            )
            .bind(sid.to_string())
            .bind(org_id.to_string())
            .fetch_one(pool)
            .await?;
            Ok(if exists.0 != 0 { Some(sid) } else { None })
        }
        ("franchise" | "org", _, hint) if hint.is_some() => {
            let sid = hint.unwrap();
            let exists: (i64,) = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM stores WHERE id = ? AND org_id = ?) AS ex",
            )
            .bind(sid.to_string())
            .bind(org_id.to_string())
            .fetch_one(pool)
            .await?;
            Ok(if exists.0 != 0 { Some(sid) } else { None })
        }
        _ => {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM stores WHERE org_id = ? AND status = 'active' ORDER BY created_at LIMIT 1",
            )
            .bind(org_id.to_string())
            .fetch_optional(pool)
            .await?;
            Ok(row.and_then(|(s,)| Uuid::parse_str(&s).ok()))
        }
    }
}

/// Insert device and return id, org_id, store_id.
pub async fn create_device(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    device_label: Option<&str>,
    hardware_fingerprint: Option<&str>,
    device_name: Option<&str>,
    is_primary: bool,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO devices (id, org_id, store_id, device_label, hardware_fingerprint, device_name, is_primary, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'active')
        "#,
    )
    .bind(id.to_string())
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(device_label)
    .bind(hardware_fingerprint)
    .bind(device_name)
    .bind(is_primary)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Update device display name and primary flag (from device_updated event or activate).
pub async fn update_device_name_primary(
    pool: &MySqlPool,
    device_id: Uuid,
    device_name: Option<&str>,
    is_primary: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE devices SET device_name = ?, is_primary = ? WHERE id = ?
        "#,
    )
    .bind(device_name)
    .bind(is_primary)
    .bind(device_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert an activation key (caller provides key_hash = SHA-256 of raw secret).
/// Returns the new key id. Raw secret must be shown once to the operator and never stored.
pub async fn create_activation_key(
    pool: &MySqlPool,
    org_id: Uuid,
    scope_type: &str,
    scope_id: Option<Uuid>,
    key_hash: &str,
    max_uses: Option<i32>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let is_multi_use = max_uses.map(|m| m > 1).unwrap_or(false);
    sqlx::query(
        r#"
        INSERT INTO device_activation_keys (id, org_id, scope_type, scope_id, key_hash, is_multi_use, max_uses, uses_count, expires_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(org_id.to_string())
    .bind(scope_type)
    .bind(scope_id.map(|u| u.to_string()))
    .bind(key_hash)
    .bind(is_multi_use)
    .bind(max_uses)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Increment uses_count for activation key.
pub async fn increment_activation_key_uses(pool: &MySqlPool, key_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE device_activation_keys SET uses_count = uses_count + 1 WHERE id = ?",
    )
    .bind(key_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert device_token (store token_hash, not raw token).
pub async fn create_device_token(
    pool: &MySqlPool,
    device_id: Uuid,
    token_hash: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO device_tokens (id, device_id, token_hash)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(device_id.to_string())
    .bind(token_hash)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Device identity from valid token (for sync endpoints).
#[derive(Debug, Clone)]
pub struct DeviceIdentity {
    pub device_id: Uuid,
    pub org_id: Uuid,
    pub store_id: Uuid,
}

/// Validate device token (hash): return device identity if token is valid and not revoked.
pub async fn validate_device_token(
    pool: &MySqlPool,
    token_hash: &str,
) -> Result<Option<DeviceIdentity>, sqlx::Error> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT d.id, d.org_id, d.store_id
        FROM devices d
        JOIN device_tokens t ON t.device_id = d.id
        WHERE t.token_hash = ? AND t.revoked_at IS NULL AND d.status = 'active'
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(a, b, c)| {
        let device_id = Uuid::parse_str(&a).ok()?;
        let org_id = Uuid::parse_str(&b).ok()?;
        let store_id = Uuid::parse_str(&c).ok()?;
        Some(DeviceIdentity {
            device_id,
            org_id,
            store_id,
        })
    }))
}

/// Create device_sync_state row for new device.
pub async fn create_device_sync_state(
    pool: &MySqlPool,
    device_id: Uuid,
    org_id: Uuid,
    store_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT IGNORE INTO device_sync_state (device_id, org_id, store_id, last_ack_seq)
        VALUES (?, ?, ?, NULL)
        "#,
    )
    .bind(device_id.to_string())
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}
