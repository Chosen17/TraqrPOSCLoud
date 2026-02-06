use crate::DbPool;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Clone)]
pub struct DeliveryIntegrationRow {
    pub id: String,
    pub org_id: String,
    pub store_id: String,
    pub provider: String,
    pub status: String,
    pub api_key_enc: Option<String>,
    pub client_id_enc: Option<String>,
    pub client_secret_enc: Option<String>,
    pub access_token_enc: Option<String>,
    pub refresh_token_enc: Option<String>,
    pub token_expires_at: Option<chrono::NaiveDateTime>,
    pub webhook_secret_enc: Option<String>,
    pub provider_store_reference: Option<String>,
    pub last_sync_at: Option<chrono::NaiveDateTime>,
    pub last_error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewDeliveryIntegration<'a> {
    pub org_id: &'a str,
    pub store_id: &'a str,
    pub provider: &'a str,
    pub status: &'a str,
    pub api_key_enc: Option<&'a str>,
    pub client_id_enc: Option<&'a str>,
    pub client_secret_enc: Option<&'a str>,
    pub access_token_enc: Option<&'a str>,
    pub refresh_token_enc: Option<&'a str>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub webhook_secret_enc: Option<&'a str>,
    pub provider_store_reference: Option<&'a str>,
}

pub async fn upsert_integration(
    pool: &DbPool,
    new: NewDeliveryIntegration<'_>,
) -> Result<DeliveryIntegrationRow, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    // MySQL doesn't have native upsert with returning; do insert .. on duplicate key update and select.
    sqlx::query(
        r#"
        INSERT INTO delivery_integrations (
          id, org_id, store_id, provider, status,
          api_key_enc, client_id_enc, client_secret_enc,
          access_token_enc, refresh_token_enc, token_expires_at,
          webhook_secret_enc, provider_store_reference
        )
        VALUES (?, ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?,
                ?, ?)
        ON DUPLICATE KEY UPDATE
          status = VALUES(status),
          api_key_enc = VALUES(api_key_enc),
          client_id_enc = VALUES(client_id_enc),
          client_secret_enc = VALUES(client_secret_enc),
          access_token_enc = VALUES(access_token_enc),
          refresh_token_enc = VALUES(refresh_token_enc),
          token_expires_at = VALUES(token_expires_at),
          webhook_secret_enc = VALUES(webhook_secret_enc),
          provider_store_reference = VALUES(provider_store_reference),
          last_error_message = NULL
        "#,
    )
    .bind(&id)
    .bind(new.org_id)
    .bind(new.store_id)
    .bind(new.provider)
    .bind(new.status)
    .bind(new.api_key_enc)
    .bind(new.client_id_enc)
    .bind(new.client_secret_enc)
    .bind(new.access_token_enc)
    .bind(new.refresh_token_enc)
    .bind(new.token_expires_at.map(|dt| dt.naive_utc()))
    .bind(new.webhook_secret_enc)
    .bind(new.provider_store_reference)
    .execute(pool)
    .await?;

    // Fetch the row (unique by store_id, provider).
    find_integration_by_store_and_provider(pool, new.store_id, new.provider).await
}

pub async fn update_integration_status(
    pool: &DbPool,
    id: &str,
    status: &str,
    last_error_message: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE delivery_integrations
        SET status = ?, last_error_message = ?, last_sync_at = CASE WHEN ? IS NULL THEN last_sync_at ELSE last_sync_at END
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(last_error_message)
    .bind(last_error_message)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn touch_integration_last_sync(
    pool: &DbPool,
    id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE delivery_integrations
        SET last_sync_at = CURRENT_TIMESTAMP(3)
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_integration_by_store_and_provider(
    pool: &DbPool,
    store_id: &str,
    provider: &str,
) -> Result<DeliveryIntegrationRow, sqlx::Error> {
    sqlx::query_as::<_, DeliveryIntegrationRow>(
        r#"
        SELECT
          id, org_id, store_id, provider, status,
          api_key_enc, client_id_enc, client_secret_enc,
          access_token_enc, refresh_token_enc, token_expires_at,
          webhook_secret_enc, provider_store_reference,
          last_sync_at, last_error_message
        FROM delivery_integrations
        WHERE store_id = ? AND provider = ?
        "#,
    )
    .bind(store_id)
    .bind(provider)
    .fetch_one(pool)
    .await
}

pub async fn find_integration_by_provider_store_reference(
    pool: &DbPool,
    provider: &str,
    provider_store_reference: &str,
) -> Result<Option<DeliveryIntegrationRow>, sqlx::Error> {
    sqlx::query_as::<_, DeliveryIntegrationRow>(
        r#"
        SELECT
          id, org_id, store_id, provider, status,
          api_key_enc, client_id_enc, client_secret_enc,
          access_token_enc, refresh_token_enc, token_expires_at,
          webhook_secret_enc, provider_store_reference,
          last_sync_at, last_error_message
        FROM delivery_integrations
        WHERE provider = ? AND provider_store_reference = ?
        "#,
    )
    .bind(provider)
    .bind(provider_store_reference)
    .fetch_optional(pool)
    .await
}

#[derive(Debug, FromRow, Clone)]
pub struct DeliveryOrderRow {
    pub id: String,
    pub org_id: String,
    pub store_id: String,
    pub integration_id: String,
    pub provider: String,
    pub provider_order_id: String,
    pub status: String,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub delivery_address: Option<Value>,
    pub items: Value,
    pub subtotal_cents: Option<i64>,
    pub tax_cents: Option<i64>,
    pub delivery_fee_cents: Option<i64>,
    pub total_cents: Option<i64>,
    pub notes: Option<String>,
    pub raw_payload: Value,
    pub received_at: chrono::NaiveDateTime,
}

#[derive(Debug)]
pub struct NewDeliveryOrder<'a> {
    pub org_id: &'a str,
    pub store_id: &'a str,
    pub integration_id: &'a str,
    pub provider: &'a str,
    pub provider_order_id: &'a str,
    pub status: &'a str,
    pub customer_name: Option<&'a str>,
    pub customer_phone: Option<&'a str>,
    pub delivery_address: Option<&'a Value>,
    pub items: &'a Value,
    pub subtotal_cents: Option<i64>,
    pub tax_cents: Option<i64>,
    pub delivery_fee_cents: Option<i64>,
    pub total_cents: Option<i64>,
    pub notes: Option<&'a str>,
    pub raw_payload: &'a Value,
    pub received_at: DateTime<Utc>,
}

pub async fn insert_delivery_order(
    pool: &DbPool,
    order: NewDeliveryOrder<'_>,
) -> Result<DeliveryOrderRow, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO delivery_orders (
          id, org_id, store_id, integration_id,
          provider, provider_order_id, status,
          customer_name, customer_phone,
          delivery_address, items,
          subtotal_cents, tax_cents, delivery_fee_cents, total_cents,
          notes, raw_payload, received_at
        )
        VALUES (?, ?, ?, ?,
                ?, ?, ?,
                ?, ?,
                ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?)
        ON DUPLICATE KEY UPDATE
          status = VALUES(status),
          customer_name = VALUES(customer_name),
          customer_phone = VALUES(customer_phone),
          delivery_address = VALUES(delivery_address),
          items = VALUES(items),
          subtotal_cents = VALUES(subtotal_cents),
          tax_cents = VALUES(tax_cents),
          delivery_fee_cents = VALUES(delivery_fee_cents),
          total_cents = VALUES(total_cents),
          notes = VALUES(notes),
          raw_payload = VALUES(raw_payload),
          received_at = VALUES(received_at)
        "#,
    )
    .bind(&id)
    .bind(order.org_id)
    .bind(order.store_id)
    .bind(order.integration_id)
    .bind(order.provider)
    .bind(order.provider_order_id)
    .bind(order.status)
    .bind(order.customer_name)
    .bind(order.customer_phone)
    .bind(order.delivery_address)
    .bind(order.items)
    .bind(order.subtotal_cents)
    .bind(order.tax_cents)
    .bind(order.delivery_fee_cents)
    .bind(order.total_cents)
    .bind(order.notes)
    .bind(order.raw_payload)
    .bind(order.received_at.naive_utc())
    .execute(pool)
    .await?;

    get_delivery_order_by_provider_and_id(pool, order.provider, order.provider_order_id).await
}

pub async fn get_delivery_order_by_provider_and_id(
    pool: &DbPool,
    provider: &str,
    provider_order_id: &str,
) -> Result<DeliveryOrderRow, sqlx::Error> {
    sqlx::query_as::<_, DeliveryOrderRow>(
        r#"
        SELECT
          id, org_id, store_id, integration_id,
          provider, provider_order_id, status,
          customer_name, customer_phone,
          delivery_address, items,
          subtotal_cents, tax_cents, delivery_fee_cents, total_cents,
          notes, raw_payload, received_at
        FROM delivery_orders
        WHERE provider = ? AND provider_order_id = ?
        "#,
    )
    .bind(provider)
    .bind(provider_order_id)
    .fetch_one(pool)
    .await
}

pub async fn list_delivery_orders_for_store_since(
    pool: &DbPool,
    store_id: &str,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<DeliveryOrderRow>, sqlx::Error> {
    if let Some(since) = since {
        sqlx::query_as::<_, DeliveryOrderRow>(
            r#"
            SELECT
              id, org_id, store_id, integration_id,
              provider, provider_order_id, status,
              customer_name, customer_phone,
              delivery_address, items,
              subtotal_cents, tax_cents, delivery_fee_cents, total_cents,
              notes, raw_payload, received_at
            FROM delivery_orders
            WHERE store_id = ? AND received_at >= ?
            ORDER BY received_at ASC
            "#,
        )
        .bind(store_id)
        .bind(since.naive_utc())
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, DeliveryOrderRow>(
            r#"
            SELECT
              id, org_id, store_id, integration_id,
              provider, provider_order_id, status,
              customer_name, customer_phone,
              delivery_address, items,
              subtotal_cents, tax_cents, delivery_fee_cents, total_cents,
              notes, raw_payload, received_at
            FROM delivery_orders
            WHERE store_id = ?
            ORDER BY received_at DESC
            LIMIT 100
            "#,
        )
        .bind(store_id)
        .fetch_all(pool)
        .await
    }
}

#[derive(Debug)]
pub struct NewDeliveryIntegrationLog<'a> {
    pub provider: &'a str,
    pub store_id: Option<&'a str>,
    pub integration_id: Option<&'a str>,
    pub request_url: Option<&'a str>,
    pub request_method: Option<&'a str>,
    pub request_payload: Option<&'a Value>,
    pub response_status: Option<i32>,
    pub response_payload: Option<&'a Value>,
    pub error_message: Option<&'a str>,
}

pub async fn insert_delivery_log(
    pool: &DbPool,
    log: NewDeliveryIntegrationLog<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO delivery_integration_logs (
          provider, store_id, integration_id,
          request_url, request_method, request_payload,
          response_status, response_payload, error_message
        )
        VALUES (?, ?, ?,
                ?, ?, ?,
                ?, ?, ?)
        "#,
    )
    .bind(log.provider)
    .bind(log.store_id)
    .bind(log.integration_id)
    .bind(log.request_url)
    .bind(log.request_method)
    .bind(log.request_payload)
    .bind(log.response_status)
    .bind(log.response_payload)
    .bind(log.error_message)
    .execute(pool)
    .await?;
    Ok(())
}

