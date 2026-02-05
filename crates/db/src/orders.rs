//! Orders read model: project device_event_log into orders, order_items, transactions, receipts.
//! Keeps POS local ids (e.g. event_body.order_id -> orders.local_order_id) so the portal can
//! reference them when building void_order / refund_order commands.

use sqlx::MySqlPool;
use uuid::Uuid;

/// Upsert order by (store_id, device_id, local_order_id). Sets total_cents and occurred_at.
/// Call get_order_id_by_local after this to get the cloud order id.
pub async fn upsert_order(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    device_id: Uuid,
    local_order_id: &str,
    total_cents: Option<i64>,
    occurred_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO orders (id, org_id, store_id, device_id, local_order_id, status, total_cents, occurred_at)
        VALUES (?, ?, ?, ?, ?, 'open', ?, ?)
        ON DUPLICATE KEY UPDATE total_cents = COALESCE(VALUES(total_cents), total_cents), occurred_at = VALUES(occurred_at)
        "#,
    )
    .bind(id.to_string())
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(local_order_id)
    .bind(total_cents)
    .bind(occurred_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Link receipts that have this local_order_id but no order_id yet (e.g. receipt_created arrived before order_created).
pub async fn backfill_receipt_order_id(
    pool: &MySqlPool,
    store_id: Uuid,
    device_id: Uuid,
    local_order_id: &str,
    order_id: Uuid,
) -> Result<(), sqlx::Error> {
    if local_order_id.is_empty() {
        return Ok(());
    }
    sqlx::query(
        r#"
        UPDATE receipts SET order_id = ? WHERE store_id = ? AND device_id = ? AND local_order_id = ? AND order_id IS NULL
        "#,
    )
    .bind(order_id.to_string())
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(local_order_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get cloud order id by POS local order id.
pub async fn get_order_id_by_local(
    pool: &MySqlPool,
    store_id: Uuid,
    device_id: Uuid,
    local_order_id: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM orders WHERE store_id = ? AND device_id = ? AND local_order_id = ?",
    )
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(local_order_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(s,)| Uuid::parse_str(&s).ok()))
}

/// Insert order_item for an order (cloud order id).
pub async fn insert_order_item(
    pool: &MySqlPool,
    order_id: Uuid,
    local_item_id: Option<&str>,
    product_ref: Option<&str>,
    quantity: f64,
    unit_price_cents: Option<i64>,
    line_total_cents: Option<i64>,
) -> Result<(), sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO order_items (id, order_id, local_item_id, product_ref, quantity, unit_price_cents, line_total_cents)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(order_id.to_string())
    .bind(local_item_id)
    .bind(product_ref)
    .bind(quantity)
    .bind(unit_price_cents)
    .bind(line_total_cents)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert transaction (idempotent by local_transaction_id).
pub async fn upsert_transaction(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    device_id: Uuid,
    order_id: Option<Uuid>,
    local_transaction_id: &str,
    kind: &str,
    amount_cents: i64,
    occurred_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO transactions (org_id, store_id, device_id, order_id, local_transaction_id, kind, amount_cents, occurred_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE order_id = COALESCE(VALUES(order_id), order_id), amount_cents = VALUES(amount_cents), kind = VALUES(kind), occurred_at = VALUES(occurred_at)
        "#,
    )
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(order_id.map(|u| u.to_string()))
    .bind(local_transaction_id)
    .bind(kind)
    .bind(amount_cents)
    .bind(occurred_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get cloud transaction id by POS local transaction id.
pub async fn get_transaction_id_by_local(
    pool: &MySqlPool,
    store_id: Uuid,
    device_id: Uuid,
    local_transaction_id: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM transactions WHERE store_id = ? AND device_id = ? AND local_transaction_id = ?",
    )
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(local_transaction_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(s,)| Uuid::parse_str(&s).ok()))
}

/// Insert receipt (idempotent by local_receipt_id). Stores local_order_id so we can link
/// when order_created is processed later, and so order detail can fetch by (store, device, local_order_id).
pub async fn upsert_receipt(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    device_id: Uuid,
    order_id: Option<Uuid>,
    transaction_id: Option<Uuid>,
    local_order_id: &str,
    local_receipt_id: &str,
    occurred_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO receipts (org_id, store_id, device_id, order_id, local_order_id, transaction_id, local_receipt_id, occurred_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
          order_id = COALESCE(VALUES(order_id), order_id),
          local_order_id = COALESCE(VALUES(local_order_id), local_order_id),
          transaction_id = COALESCE(VALUES(transaction_id), transaction_id),
          occurred_at = VALUES(occurred_at)
        "#,
    )
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(order_id.map(|u| u.to_string()))
    .bind(if local_order_id.is_empty() { None::<String> } else { Some(local_order_id.to_string()) })
    .bind(transaction_id.map(|u| u.to_string()))
    .bind(local_receipt_id)
    .bind(occurred_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert order_event (append-only).
pub async fn insert_order_event(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    order_id: Uuid,
    event_type: &str,
    event_body: &serde_json::Value,
    occurred_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO order_events (id, org_id, store_id, order_id, event_type, event_body, occurred_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(order_id.to_string())
    .bind(event_type)
    .bind(event_body)
    .bind(occurred_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Normalise order_id from event body (POS may send string or number).
fn local_order_id_from_body(event_body: &serde_json::Value) -> String {
    match event_body.get("order_id") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Project a single event into the orders read model. Keeps local_order_id from event_body
/// so the portal can use it for void_order / refund_order command bodies.
/// Tolerates missing or malformed event_body fields (no-op or partial update).
pub async fn project_event_to_orders(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    device_id: Uuid,
    event_type: &str,
    event_body: &serde_json::Value,
    occurred_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    let local_order_id = local_order_id_from_body(event_body);
    if local_order_id.is_empty() {
        return Ok(());
    }

    match event_type {
        "order_created" => {
            let total_cents = event_body
                .get("total_cents")
                .or(event_body.get("total"))
                .and_then(|v| v.as_i64());
            upsert_order(
                pool,
                org_id,
                store_id,
                device_id,
                &local_order_id,
                total_cents,
                occurred_at,
            )
            .await?;
            let order_id = match get_order_id_by_local(pool, store_id, device_id, &local_order_id).await? {
                Some(id) => id,
                None => return Ok(()),
            };
            let _ = backfill_receipt_order_id(pool, store_id, device_id, &local_order_id, order_id).await;
            let items = event_body
                .get("items")
                .or(event_body.get("line_items"))
                .and_then(|v| v.as_array());
            if let Some(items) = items {
                for item in items {
                    let qty = item
                        .get("quantity")
                        .and_then(|v| v.as_f64())
                        .or_else(|| item.get("qty").and_then(|v| v.as_f64()))
                        .unwrap_or(1.0);
                    let unit = item
                        .get("unit_price_cents")
                        .or(item.get("price_pence"))
                        .or(item.get("unit_price"))
                        .and_then(|v| v.as_i64())
                        .or_else(|| item.get("price").and_then(|v| v.as_f64()).map(|p| (p * 100.0) as i64));
                    let line = item
                        .get("line_total_cents")
                        .or(item.get("line_total"))
                        .and_then(|v| v.as_i64())
                        .or_else(|| item.get("line_total").and_then(|v| v.as_f64()).map(|p| (p * 100.0) as i64));
                    let local_item_id = item
                        .get("id")
                        .or(item.get("item_id"))
                        .or(item.get("local_item_id"))
                        .and_then(|v| v.as_str());
                    let product_ref = item
                        .get("product_ref")
                        .or(item.get("product_id"))
                        .or(item.get("menu_item_id"))
                        .or(item.get("name"))
                        .or(item.get("product_name"))
                        .and_then(|v| v.as_str());
                    let _ = insert_order_item(
                        pool,
                        order_id,
                        local_item_id,
                        product_ref,
                        qty,
                        unit,
                        line,
                    )
                    .await;
                }
            }
            let _ = insert_order_event(
                pool,
                org_id,
                store_id,
                order_id,
                event_type,
                event_body,
                occurred_at,
            )
            .await;
        }
        "order_updated" => {
            let order_id = match get_order_id_by_local(pool, store_id, device_id, &local_order_id).await? {
                Some(id) => id,
                None => return Ok(()),
            };
            let items = event_body
                .get("items")
                .or(event_body.get("line_items"))
                .and_then(|v| v.as_array());
            if let Some(items) = items {
                for item in items {
                    let qty = item
                        .get("quantity")
                        .and_then(|v| v.as_f64())
                        .or_else(|| item.get("qty").and_then(|v| v.as_f64()))
                        .unwrap_or(1.0);
                    let unit = item
                        .get("unit_price_cents")
                        .or(item.get("price_pence"))
                        .or(item.get("unit_price"))
                        .and_then(|v| v.as_i64())
                        .or_else(|| item.get("price").and_then(|v| v.as_f64()).map(|p| (p * 100.0) as i64));
                    let line = item
                        .get("line_total_cents")
                        .or(item.get("line_total"))
                        .and_then(|v| v.as_i64())
                        .or_else(|| item.get("line_total").and_then(|v| v.as_f64()).map(|p| (p * 100.0) as i64));
                    let local_item_id = item
                        .get("id")
                        .or(item.get("item_id"))
                        .or(item.get("local_item_id"))
                        .and_then(|v| v.as_str());
                    let product_ref = item
                        .get("product_ref")
                        .or(item.get("product_id"))
                        .or(item.get("menu_item_id"))
                        .or(item.get("name"))
                        .or(item.get("product_name"))
                        .and_then(|v| v.as_str());
                    let _ = insert_order_item(
                        pool,
                        order_id,
                        local_item_id,
                        product_ref,
                        qty,
                        unit,
                        line,
                    )
                    .await;
                }
            }
            let _ = insert_order_event(
                pool,
                org_id,
                store_id,
                order_id,
                event_type,
                event_body,
                occurred_at,
            )
            .await;
        }
        "transaction_completed" => {
            let local_tx_id = event_body
                .get("transaction_id")
                .or(event_body.get("local_transaction_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if local_tx_id.is_empty() {
                return Ok(());
            }
            let order_id = get_order_id_by_local(pool, store_id, device_id, &local_order_id).await?;
            let amount_cents = event_body
                .get("amount_cents")
                .or(event_body.get("amount"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let kind = event_body
                .get("kind")
                .or(event_body.get("payment_method"))
                .or(event_body.get("provider"))
                .and_then(|v| v.as_str())
                .unwrap_or("payment");
            upsert_transaction(
                pool,
                org_id,
                store_id,
                device_id,
                order_id,
                local_tx_id,
                kind,
                amount_cents,
                occurred_at,
            )
            .await?;
            if let Some(oid) = order_id {
                let _ = insert_order_event(
                    pool,
                    org_id,
                    store_id,
                    oid,
                    event_type,
                    event_body,
                    occurred_at,
                )
                .await;
            }
        }
        "receipt_created" => {
            let local_receipt_id = event_body
                .get("receipt_id")
                .or(event_body.get("local_receipt_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let local_tx_id = event_body
                .get("transaction_id")
                .or(event_body.get("local_transaction_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let order_id = get_order_id_by_local(pool, store_id, device_id, &local_order_id).await?;
            let transaction_id = if local_tx_id.is_empty() {
                None
            } else {
                get_transaction_id_by_local(pool, store_id, device_id, local_tx_id).await?
            };
            upsert_receipt(
                pool,
                org_id,
                store_id,
                device_id,
                order_id,
                transaction_id,
                &local_order_id,
                local_receipt_id,
                occurred_at,
            )
            .await?;
            if let Some(oid) = order_id {
                let _ = insert_order_event(
                    pool,
                    org_id,
                    store_id,
                    oid,
                    event_type,
                    event_body,
                    occurred_at,
                )
                .await;
            }
        }
        _ => {}
    }
    Ok(())
}
