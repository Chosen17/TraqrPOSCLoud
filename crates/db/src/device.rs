//! Device activation: lookup activation key, create device, issue token.

use sqlx::PgPool;
use uuid::Uuid;

/// Activation key row (lookup by key_hash).
#[derive(Debug, sqlx::FromRow)]
pub struct ActivationKeyRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub scope_type: String,
    pub scope_id: Option<Uuid>,
    pub is_multi_use: bool,
    pub max_uses: Option<i32>,
    pub uses_count: i32,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Find activation key by SHA-256 hash of the key (key_hash).
/// Caller hashes the raw activation_key before calling.
pub async fn find_activation_key_by_hash(
    pool: &PgPool,
    key_hash: &str,
) -> Result<Option<ActivationKeyRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, org_id, scope_type, scope_id, is_multi_use, max_uses, uses_count, expires_at, revoked_at
        FROM device_activation_keys
        WHERE key_hash = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await
}

/// Resolve store_id for activation: scope_type store -> scope_id; franchise/org -> use store_hint or first store.
pub async fn resolve_store_for_activation(
    pool: &PgPool,
    org_id: Uuid,
    scope_type: &str,
    scope_id: Option<Uuid>,
    store_hint: Option<Uuid>,
) -> Result<Option<Uuid>, sqlx::Error> {
    match (scope_type, scope_id, store_hint) {
        ("store", Some(sid), _) => {
            let exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM stores WHERE id = $1 AND org_id = $2)",
            )
            .bind(sid)
            .bind(org_id)
            .fetch_one(pool)
            .await?;
            Ok(if exists { Some(sid) } else { None })
        }
        ("franchise" | "org", _, hint) if hint.is_some() => {
            let sid = hint.unwrap();
            let exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM stores WHERE id = $1 AND org_id = $2)",
            )
            .bind(sid)
            .bind(org_id)
            .fetch_one(pool)
            .await?;
            Ok(if exists { Some(sid) } else { None })
        }
        _ => {
            let row: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM stores WHERE org_id = $1 AND status = 'active' ORDER BY created_at LIMIT 1",
            )
            .bind(org_id)
            .fetch_optional(pool)
            .await?;
            Ok(row.map(|r| r.0))
        }
    }
}

/// Insert device and return id, org_id, store_id.
pub async fn create_device(
    pool: &PgPool,
    org_id: Uuid,
    store_id: Uuid,
    device_label: Option<&str>,
    hardware_fingerprint: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO devices (id, org_id, store_id, device_label, hardware_fingerprint, status)
        VALUES ($1, $2, $3, $4, $5, 'active')
        "#,
    )
    .bind(id)
    .bind(org_id)
    .bind(store_id)
    .bind(device_label)
    .bind(hardware_fingerprint)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Increment uses_count for activation key.
pub async fn increment_activation_key_uses(pool: &PgPool, key_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE device_activation_keys SET uses_count = uses_count + 1 WHERE id = $1",
    )
    .bind(key_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert device_token (store token_hash, not raw token).
pub async fn create_device_token(
    pool: &PgPool,
    device_id: Uuid,
    token_hash: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO device_tokens (id, device_id, token_hash)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(id)
    .bind(device_id)
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
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<DeviceIdentity>, sqlx::Error> {
    let row: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        r#"
        SELECT d.id, d.org_id, d.store_id
        FROM devices d
        JOIN device_tokens t ON t.device_id = d.id
        WHERE t.token_hash = $1 AND t.revoked_at IS NULL AND d.status = 'active'
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(device_id, org_id, store_id)| DeviceIdentity {
        device_id,
        org_id,
        store_id,
    }))
}

/// Create device_sync_state row for new device.
pub async fn create_device_sync_state(
    pool: &PgPool,
    device_id: Uuid,
    org_id: Uuid,
    store_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO device_sync_state (device_id, org_id, store_id, last_ack_seq)
        VALUES ($1, $2, $3, NULL)
        ON CONFLICT (device_id) DO NOTHING
        "#,
    )
    .bind(device_id)
    .bind(org_id)
    .bind(store_id)
    .execute(pool)
    .await?;
    Ok(())
}
