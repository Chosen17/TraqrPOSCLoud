//! Sync: idempotent event insert, device_sync_state update, command fetch/ack.

use sqlx::PgPool;
use uuid::Uuid;

/// Insert event idempotently (ON CONFLICT DO NOTHING on (device_id, event_id)).
/// Returns whether the row was actually inserted (true) or duplicate (false).
pub async fn insert_event_idempotent(
    pool: &PgPool,
    org_id: Uuid,
    store_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
    seq: Option<i64>,
    event_type: &str,
    event_body: &serde_json::Value,
    occurred_at: chrono::DateTime<chrono::Utc>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO device_event_log (org_id, store_id, device_id, event_id, seq, event_type, event_body, occurred_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (device_id, event_id) DO NOTHING
        "#,
    )
    .bind(org_id)
    .bind(store_id)
    .bind(device_id)
    .bind(event_id)
    .bind(seq)
    .bind(event_type)
    .bind(event_body)
    .bind(occurred_at)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Update device_sync_state.last_ack_seq for device (only if new value is greater or null).
pub async fn update_device_sync_state_ack_seq(
    pool: &PgPool,
    device_id: Uuid,
    org_id: Uuid,
    store_id: Uuid,
    last_ack_seq: Option<i64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO device_sync_state (device_id, org_id, store_id, last_ack_seq, updated_at)
        VALUES ($1, $2, $3, $4, now())
        ON CONFLICT (device_id) DO UPDATE SET
          last_ack_seq = GREATEST(COALESCE(device_sync_state.last_ack_seq, -1), COALESCE($4, -1)),
          updated_at = now()
        "#,
    )
    .bind(device_id)
    .bind(org_id)
    .bind(store_id)
    .bind(last_ack_seq)
    .execute(pool)
    .await?;
    Ok(())
}

/// Row for a deliverable command (queued or delivered, not acked/failed/expired).
#[derive(Debug, sqlx::FromRow)]
pub struct CommandRow {
    pub command_id: Uuid,
    pub command_type: String,
    pub command_body: serde_json::Value,
    pub sensitive: bool,
}

/// Fetch deliverable commands for device (status in queued, delivered), ordered by created_at.
pub async fn fetch_deliverable_commands(
    pool: &PgPool,
    device_id: Uuid,
    limit: i64,
) -> Result<Vec<CommandRow>, sqlx::Error> {
    let rows = sqlx::query_as(
        r#"
        SELECT command_id, command_type, command_body, sensitive
        FROM device_command_queue
        WHERE device_id = $1 AND status IN ('queued', 'delivered')
        ORDER BY created_at
        LIMIT $2
        "#,
    )
    .bind(device_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Mark command as delivered (optional: set delivered_at).
pub async fn mark_command_delivered(pool: &PgPool, command_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE device_command_queue
        SET status = 'delivered', delivered_at = now()
        WHERE command_id = $1 AND status = 'queued'
        "#,
    )
    .bind(command_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Ack or fail command: set status and ack_result. Only updates if command belongs to device (tenant-safe).
pub async fn ack_command(
    pool: &PgPool,
    device_id: Uuid,
    command_id: Uuid,
    status: &str,
    ack_result: Option<&serde_json::Value>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE device_command_queue
        SET status = $2, ack_result = $3
        WHERE command_id = $4 AND device_id = $1 AND status IN ('queued', 'delivered')
        "#,
    )
    .bind(device_id)
    .bind(status)
    .bind(ack_result)
    .bind(command_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
