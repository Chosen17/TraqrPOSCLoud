//! Sync: idempotent event insert, device_sync_state update, command fetch/ack.

use sqlx::MySqlPool;
use uuid::Uuid;

/// Insert event idempotently (ignore duplicate on (device_id, event_id)).
/// Returns whether the row was actually inserted (true) or duplicate (false).
pub async fn insert_event_idempotent(
    pool: &MySqlPool,
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
        INSERT IGNORE INTO device_event_log (org_id, store_id, device_id, event_id, seq, event_type, event_body, occurred_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(event_id.to_string())
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
    pool: &MySqlPool,
    device_id: Uuid,
    org_id: Uuid,
    store_id: Uuid,
    last_ack_seq: Option<i64>,
) -> Result<(), sqlx::Error> {
    let seq_val = last_ack_seq.unwrap_or(-1);
    sqlx::query(
        r#"
        INSERT INTO device_sync_state (device_id, org_id, store_id, last_ack_seq, updated_at)
        VALUES (?, ?, ?, ?, NOW())
        ON DUPLICATE KEY UPDATE
          last_ack_seq = GREATEST(COALESCE(device_sync_state.last_ack_seq, -1), ?),
          updated_at = NOW()
        "#,
    )
    .bind(device_id.to_string())
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(last_ack_seq)
    .bind(seq_val)
    .execute(pool)
    .await?;
    Ok(())
}

/// Row for a deliverable command (queued or delivered, not acked/failed/expired). command_id decoded as String from CHAR(36).
#[derive(Debug, sqlx::FromRow)]
pub struct CommandRow {
    pub command_id: String,
    pub command_type: String,
    pub command_body: serde_json::Value,
    pub sensitive: bool,
}

/// Fetch deliverable commands for device (status in queued, delivered), ordered by created_at.
pub async fn fetch_deliverable_commands(
    pool: &MySqlPool,
    device_id: Uuid,
    limit: i64,
) -> Result<Vec<CommandRow>, sqlx::Error> {
    let rows = sqlx::query_as(
        r#"
        SELECT command_id, command_type, command_body, sensitive
        FROM device_command_queue
        WHERE device_id = ? AND status IN ('queued', 'delivered')
        ORDER BY created_at
        LIMIT ?
        "#,
    )
    .bind(device_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Mark command as delivered (optional: set delivered_at).
pub async fn mark_command_delivered(pool: &MySqlPool, command_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE device_command_queue
        SET status = 'delivered', delivered_at = NOW()
        WHERE command_id = ? AND status = 'queued'
        "#,
    )
    .bind(command_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// Ack or fail command: set status and ack_result. Only updates if command belongs to device (tenant-safe).
pub async fn ack_command(
    pool: &MySqlPool,
    device_id: Uuid,
    command_id: Uuid,
    status: &str,
    ack_result: Option<&serde_json::Value>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE device_command_queue
        SET status = ?, ack_result = ?
        WHERE command_id = ? AND device_id = ? AND status IN ('queued', 'delivered')
        "#,
    )
    .bind(status)
    .bind(ack_result)
    .bind(command_id.to_string())
    .bind(device_id.to_string())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Enqueue apply_menu command to every device in the store (so cloud menu edits reach all devices).
pub async fn enqueue_apply_menu_for_store(
    pool: &MySqlPool,
    store_id: Uuid,
) -> Result<u64, sqlx::Error> {
    use crate::read_model::get_store_menu_for_sync;

    let (org_id, device_ids): (String, Vec<String>) = {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT org_id FROM stores WHERE id = ?")
                .bind(store_id.to_string())
                .fetch_optional(pool)
                .await?;
        let org_id = match row {
            Some((id,)) => id,
            None => return Ok(0),
        };
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT device_id FROM device_sync_state WHERE store_id = ?",
        )
        .bind(store_id.to_string())
        .fetch_all(pool)
        .await?;
        let device_ids: Vec<String> = rows.into_iter().map(|(d,)| d).collect();
        (org_id, device_ids)
    };

    let menu = match get_store_menu_for_sync(pool, store_id).await? {
        Some(m) => m,
        None => return Ok(0),
    };
    let body = serde_json::json!({
        "categories": menu.0,
        "items": menu.1,
    });

    let mut count = 0u64;
    for device_id in &device_ids {
        let command_id = Uuid::new_v4();
        let r = sqlx::query(
            r#"
            INSERT INTO device_command_queue (command_id, org_id, store_id, device_id, command_type, command_body, status, sensitive, created_at)
            VALUES (?, ?, ?, ?, 'apply_menu', ?, 'queued', 0, CURRENT_TIMESTAMP(3))
            "#,
        )
        .bind(command_id.to_string())
        .bind(&org_id)
        .bind(store_id.to_string())
        .bind(device_id)
        .bind(&body)
        .execute(pool)
        .await?;
        count += r.rows_affected();
    }
    Ok(count)
}
