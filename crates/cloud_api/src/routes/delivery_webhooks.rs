use axum::body::Bytes;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};

use crate::session::CurrentUser;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::{NaiveDateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{delivery_connectors, state::AppState};
use db::{
    enqueue_delivery_order_command, find_integration_by_provider_store_reference, insert_delivery_log,
    insert_delivery_order, list_delivery_orders_for_store_since, DeliveryIntegrationRow, NewDeliveryIntegrationLog,
    NewDeliveryOrder,
};
use domain::{DeliveryOrderNormalized, DeliveryOrderStatus, DeliveryProvider};

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/webhooks/just_eat", post(handle_just_eat_webhook))
        .route("/webhooks/deliveroo", post(handle_deliveroo_webhook))
        .route("/webhooks/uber_eats", post(handle_uber_eats_webhook))
        .route(
            "/portal/stores/:store_id/delivery_integrations",
            get(get_store_delivery_integrations),
        )
        .route(
            "/portal/stores/:store_id/delivery_integrations/:provider/connect",
            post(connect_integration),
        )
        .route(
            "/portal/stores/:store_id/delivery_integrations/:provider/disconnect",
            post(disconnect_integration),
        )
        .route(
            "/portal/stores/:store_id/delivery_integrations/:provider/test",
            post(test_integration),
        )
        .route(
            "/stores/:store_id/delivery_orders",
            axum::routing::get(list_store_delivery_orders),
        )
}

#[derive(Debug, Deserialize)]
struct StorePathParams {
    store_id: String,
}

#[derive(Debug, Deserialize)]
struct ProviderPath {
    store_id: String,
    provider: String,
}

async fn get_store_delivery_integrations(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StorePathParams { store_id }): Path<StorePathParams>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = state
        .db
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "database not available".to_string()))?;
    let store_uuid =
        Uuid::parse_str(&store_id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let rows = sqlx::query_as::<_, DeliveryIntegrationRow>(
        r#"
        SELECT
          id,
          org_id,
          store_id,
          provider,
          status,
          provider_store_reference,
          last_sync_at,
          last_error_message
        FROM delivery_integrations
        WHERE store_id = ?
        "#,
    )
    .bind(store_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let mut map = serde_json::Map::new();
    for r in rows {
        let key = r.provider.clone();
        map.insert(
            key,
            serde_json::json!({
                "id": r.id,
                "org_id": r.org_id,
                "store_id": r.store_id,
                "provider": r.provider,
                "status": r.status,
                "provider_store_reference": r.provider_store_reference,
                "last_sync_at": r.last_sync_at.map(|dt: NaiveDateTime| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
                "last_error_message": r.last_error_message,
            }),
        );
    }
    let response = serde_json::json!({ "integrations": map });

    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
struct ConnectBody {
    api_key: String,
    /// Provider restaurant/store identifier (e.g. restaurant_id, location_id, store_id).
    provider_store_reference: String,
    /// Optional. Required for Uber Eats: OAuth client_id (for webhook verification and GET order).
    client_id: Option<String>,
    /// Optional. Required for Uber Eats: OAuth client_secret (for X-Uber-Signature and API calls).
    client_secret: Option<String>,
}

async fn connect_integration(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ProviderPath { store_id, provider }): Path<ProviderPath>,
    Json(body): Json<ConnectBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = state
        .db
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "database not available".to_string()))?;
    let store_uuid =
        Uuid::parse_str(&store_id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let store_row: Option<(String,)> =
        sqlx::query_as("SELECT org_id FROM stores WHERE id = ?")
            .bind(store_uuid.to_string())
            .fetch_optional(db)
            .await
            .map_err(internal)?;
    let Some((org_id,)) = store_row else {
        return Err((StatusCode::NOT_FOUND, "store not found".to_string()));
    };

    let provider_enum = match provider.as_str() {
        "just_eat" => DeliveryProvider::JustEat,
        "deliveroo" => DeliveryProvider::Deliveroo,
        "uber_eats" => DeliveryProvider::UberEats,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "provider must be just_eat, deliveroo, or uber_eats".to_string(),
            ))
        }
    };

    let api_key_enc =
        crate::crypto::encrypt_secret(&body.api_key).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let client_id_enc = body
        .client_id
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| crate::crypto::encrypt_secret(s).ok());
    let client_secret_enc = body
        .client_secret
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| crate::crypto::encrypt_secret(s).ok());

    // Generate or capture a per-integration webhook secret (plain), then encrypt it for storage.
    // For Deliveroo, the "API key" we ask the user for is actually the webhook secret
    // from the Deliveroo Developer Portal; use it directly so HMAC verification matches.
    let plain_webhook_secret = if provider == "deliveroo" {
        body.api_key.clone()
    } else {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        BASE64_STANDARD.encode(bytes)
    };
    let webhook_secret_enc = crate::crypto::encrypt_secret(&plain_webhook_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let new = db::NewDeliveryIntegration {
        org_id: &org_id,
        store_id: &store_uuid.to_string(),
        provider: &provider,
        status: "pending",
        api_key_enc: Some(&api_key_enc),
        client_id_enc: client_id_enc.as_deref(),
        client_secret_enc: client_secret_enc.as_deref(),
        access_token_enc: None,
        refresh_token_enc: None,
        token_expires_at: None,
        webhook_secret_enc: Some(&webhook_secret_enc),
        provider_store_reference: Some(&body.provider_store_reference),
    };

    let row = db::upsert_integration(db, new)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let config = delivery_connectors::DeliveryIntegrationConfig {
        org_id: row.org_id.clone(),
        store_id: row.store_id.clone(),
        provider: provider_enum,
        api_key: Some(body.api_key.clone()),
        client_id: body.client_id.clone(),
        client_secret: body.client_secret.clone(),
        access_token: None,
        refresh_token: None,
        webhook_secret: Some(plain_webhook_secret.clone()),
        provider_store_reference: row.provider_store_reference.clone(),
    };

    let connector = delivery_connectors::connector_for(&config.provider);
    let callback_url = format!(
        "{}/api/webhooks/{}",
        std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "https://example.com".to_string()),
        provider
    );

    let test_res = connector.test_connection(&config).await.map_err(|e| {
        let msg = format!("test_connection failed: {:?}", e);
        (StatusCode::BAD_REQUEST, msg)
    })?;

    if !test_res.ok {
        return Err((StatusCode::BAD_REQUEST, test_res.message));
    }

    // Best-effort webhook registration; errors surfaced but keep integration as pending/error.
    if let Err(e) = connector.register_webhook(&config, &callback_url).await {
        let msg = format!("register_webhook failed: {:?}", e);
        db::update_integration_status(db, &row.id, "error", Some(&msg))
            .await
            .ok();
        return Err((StatusCode::BAD_REQUEST, msg));
    }

    db::update_integration_status(db, &row.id, "connected", None)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "Integration connected"
    })))
}

async fn disconnect_integration(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ProviderPath { store_id, provider }): Path<ProviderPath>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = state
        .db
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "database not available".to_string()))?;
    let store_uuid =
        Uuid::parse_str(&store_id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    sqlx::query(
        r#"
        UPDATE delivery_integrations
        SET status = 'disconnected',
            api_key_enc = NULL,
            client_id_enc = NULL,
            client_secret_enc = NULL,
            access_token_enc = NULL,
            refresh_token_enc = NULL,
            token_expires_at = NULL,
            webhook_secret_enc = NULL
        WHERE store_id = ? AND provider = ?
        "#,
    )
    .bind(store_uuid.to_string())
    .bind(&provider)
    .execute(db)
    .await
    .map_err(internal)?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "Integration disconnected"
    })))
}

async fn test_integration(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ProviderPath { store_id, provider }): Path<ProviderPath>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = state
        .db
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "database not available".to_string()))?;
    let store_uuid =
        Uuid::parse_str(&store_id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let row = db::find_integration_by_store_and_provider(db, &store_uuid.to_string(), &provider)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let api_key = row
        .api_key_enc
        .as_deref()
        .map(crate::crypto::decrypt_secret)
        .transpose()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let client_id = row
        .client_id_enc
        .as_deref()
        .map(crate::crypto::decrypt_secret)
        .transpose()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let client_secret = row
        .client_secret_enc
        .as_deref()
        .map(crate::crypto::decrypt_secret)
        .transpose()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let provider_enum = match provider.as_str() {
        "just_eat" => DeliveryProvider::JustEat,
        "deliveroo" => DeliveryProvider::Deliveroo,
        "uber_eats" => DeliveryProvider::UberEats,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "provider must be just_eat, deliveroo, or uber_eats".to_string(),
            ))
        }
    };

    let config = delivery_connectors::DeliveryIntegrationConfig {
        org_id: row.org_id.clone(),
        store_id: row.store_id.clone(),
        provider: provider_enum,
        api_key,
        client_id,
        client_secret,
        access_token: None,
        refresh_token: None,
        webhook_secret: None,
        provider_store_reference: row.provider_store_reference.clone(),
    };

    let connector = delivery_connectors::connector_for(&config.provider);
    let result = connector
        .test_connection(&config)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("test_connection failed: {:?}", e)))?;

    Ok(Json(serde_json::json!({
        "ok": result.ok,
        "message": result.message
    })))
}

async fn handle_just_eat_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    handle_provider_webhook(state, "just_eat", headers, body).await
}

async fn handle_deliveroo_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    handle_provider_webhook(state, "deliveroo", headers, body).await
}

async fn handle_uber_eats_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    handle_provider_webhook(state, "uber_eats", headers, body).await
}

async fn handle_provider_webhook(
    state: AppState,
    provider: &str,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state
        .db
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "database not available".to_string()))?;

    let payload: Value = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid JSON payload: {e}")))?;
    let provider_store_ref = extract_provider_store_reference(provider, &payload).ok_or((
        StatusCode::BAD_REQUEST,
        "missing restaurant/store identifier in payload".to_string(),
    ))?;

    let integration =
        find_integration_by_provider_store_reference(db, provider, provider_store_ref)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::BAD_REQUEST,
            "no matching delivery integration for this restaurant".to_string(),
        ))?;

    // Verify webhook signature using provider-specific header and stored secret.
    let webhook_secret_enc = integration
        .webhook_secret_enc
        .as_deref()
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "webhook secret not configured for integration".to_string(),
        ))?;
    let webhook_secret = crate::crypto::decrypt_secret(webhook_secret_enc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Decide verification strategy for this provider.
    let strategy = {
        let provider_enum = match provider {
            "just_eat" => DeliveryProvider::JustEat,
            "deliveroo" => DeliveryProvider::Deliveroo,
            "uber_eats" => DeliveryProvider::UberEats,
            _ => DeliveryProvider::JustEat,
        };
        let connector = delivery_connectors::connector_for(&provider_enum);
        connector.webhook_verification_strategy()
    };

    // Extract key headers for logging and verification (provider-specific).
    let (header_name, sig_header_val) = match provider {
        "deliveroo" => (
            "x-deliveroo-hmac-sha256",
            headers.get("x-deliveroo-hmac-sha256").or_else(|| headers.get("X-Deliveroo-Hmac-Sha256")).and_then(|v| v.to_str().ok()),
        ),
        "uber_eats" => (
            "x-uber-signature",
            headers.get("x-uber-signature").or_else(|| headers.get("X-Uber-Signature")).and_then(|v| v.to_str().ok()),
        ),
        _ => (
            "x-just-eat-signature",
            headers.get("x-just-eat-signature").and_then(|v| v.to_str().ok()),
        ),
    };
    let deliveroo_guid = headers.get("x-deliveroo-sequence-guid").or_else(|| headers.get("X-Deliveroo-Sequence-Guid")).and_then(|v| v.to_str().ok());
    let timestamp_header = headers
        .get("x-request-timestamp")
        .or_else(|| headers.get("date"))
        .and_then(|v| v.to_str().ok());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok());
    let request_id = headers
        .get("x-request-id")
        .or_else(|| headers.get("x-uber-eats-request-id"))
        .and_then(|v| v.to_str().ok());

    let headers_json = serde_json::json!({
        "signature_header": header_name,
        "signature_value": sig_header_val.unwrap_or(""),
        "x_deliveroo_sequence_guid": deliveroo_guid.unwrap_or(""),
        "timestamp": timestamp_header.unwrap_or(""),
        "user_agent": user_agent.unwrap_or(""),
        "request_id": request_id.unwrap_or(""),
    });

    let signature_valid = match strategy {
        delivery_connectors::WebhookVerificationStrategy::TraqrHmacSha256Hex => {
            let sig = sig_header_val.ok_or((
                StatusCode::UNAUTHORIZED,
                format!("missing webhook signature header {}", header_name),
            ))?;
            verify_traqr_webhook_secret(&webhook_secret, &body, sig)
        }
        delivery_connectors::WebhookVerificationStrategy::DeliverooHmacSha256 => {
            let guid = deliveroo_guid.ok_or((
                StatusCode::UNAUTHORIZED,
                "missing X-Deliveroo-Sequence-Guid".to_string(),
            ))?;
            let sig = sig_header_val.ok_or((
                StatusCode::UNAUTHORIZED,
                "missing X-Deliveroo-Hmac-Sha256".to_string(),
            ))?;
            verify_deliveroo_webhook(&webhook_secret, guid, &body, sig)
        }
        delivery_connectors::WebhookVerificationStrategy::UberEatsHmacSha256Hex => {
            let uber_secret = integration
                .client_secret_enc
                .as_deref()
                .and_then(|enc| crate::crypto::decrypt_secret(enc).ok())
                .ok_or((
                    StatusCode::UNAUTHORIZED,
                    "Uber Eats integration missing client_secret for webhook verification".to_string(),
                ))?;
            let sig = sig_header_val.ok_or((
                StatusCode::UNAUTHORIZED,
                "missing X-Uber-Signature".to_string(),
            ))?;
            verify_uber_eats_webhook(&uber_secret, &body, sig)
        }
        _ => true,
    };

    if !signature_valid {
        let wrapped_request = serde_json::json!({
            "body": payload,
            "headers": headers_json,
        });
        let log = NewDeliveryIntegrationLog {
            provider,
            store_id: Some(&integration.store_id),
            integration_id: Some(&integration.id),
            request_url: None,
            request_method: Some("POST"),
            request_payload: Some(&wrapped_request),
            response_status: Some(StatusCode::UNAUTHORIZED.as_u16() as i32),
            response_payload: None,
            error_message: Some("invalid webhook signature"),
        };
        let _ = insert_delivery_log(db, log).await;
        return Err((StatusCode::UNAUTHORIZED, "invalid webhook signature".to_string()));
    }

    let org_id =
        Uuid::parse_str(&integration.org_id).map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "invalid org_id".to_string()))?;
    let store_id = Uuid::parse_str(&integration.store_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "invalid store_id".to_string()))?;

    let normalized = if provider == "uber_eats" {
        let resource_href = payload.get("resource_href").and_then(|v| v.as_str());
        let client_id = integration
            .client_id_enc
            .as_deref()
            .and_then(|enc| crate::crypto::decrypt_secret(enc).ok());
        let client_secret = integration
            .client_secret_enc
            .as_deref()
            .and_then(|enc| crate::crypto::decrypt_secret(enc).ok());
        if let (Some(href), Some(ref cid), Some(ref csec)) = (resource_href, client_id, client_secret) {
            match delivery_connectors::uber_eats::fetch_uber_eats_order_full(
                cid,
                csec,
                href,
                store_id,
                org_id,
            )
            .await
            {
                Ok(norm) => norm,
                Err(e) => {
                    tracing::warn!(
                        "Uber Eats fetch order failed: {:?}, falling back to webhook payload",
                        e
                    );
                    normalize_order(provider, &payload, org_id, store_id).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            format!("failed to normalize order payload: {}", e),
                        )
                    })?
                }
            }
        } else {
            normalize_order(provider, &payload, org_id, store_id).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("failed to normalize order payload: {}", e),
                )
            })?
        }
    } else {
        normalize_order(provider, &payload, org_id, store_id).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("failed to normalize order payload: {}", e),
            )
        })?
    };

    let delivery_address_value = normalized
        .delivery_address
        .as_ref()
        .map(|addr| {
            serde_json::json!({
                "line1": addr.line1,
                "line2": addr.line2,
                "city": addr.city,
                "postcode": addr.postcode,
                "country": addr.country,
            })
        });
    let items_value = serde_json::to_value(&normalized.items).unwrap_or(Value::Null);

    let order = NewDeliveryOrder {
        org_id: &integration.org_id,
        store_id: &integration.store_id,
        integration_id: &integration.id,
        provider,
        provider_order_id: &normalized.external_order_id,
        status: match normalized.status {
            DeliveryOrderStatus::Pending => "pending",
            DeliveryOrderStatus::Accepted => "accepted",
            DeliveryOrderStatus::Rejected => "rejected",
            DeliveryOrderStatus::Cancelled => "cancelled",
            DeliveryOrderStatus::Ready => "ready",
            DeliveryOrderStatus::Collected => "collected",
            DeliveryOrderStatus::Delivered => "delivered",
        },
        customer_name: normalized.customer.as_ref().and_then(|c| c.name.as_deref()),
        customer_phone: normalized.customer.as_ref().and_then(|c| c.phone.as_deref()),
        delivery_address: delivery_address_value.as_ref(),
        items: &items_value,
        subtotal_cents: None,
        tax_cents: None,
        delivery_fee_cents: None,
        total_cents: Some((normalized.total * 100.0).round() as i64),
        notes: normalized.notes.as_deref(),
        raw_payload: &payload,
        received_at: normalized.received_at.unwrap_or_else(Utc::now),
    };

    let _saved = insert_delivery_order(db, order)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Emit POS-facing command using normalized payload.
    let pos_payload = serde_json::to_value(&normalized)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    enqueue_delivery_order_command(db, org_id, store_id, &pos_payload)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    db::touch_integration_last_sync(db, &integration.id)
        .await
        .ok();

    // Log successful webhook processing
    let log = NewDeliveryIntegrationLog {
        provider,
        store_id: Some(&integration.store_id),
        integration_id: Some(&integration.id),
        request_url: None,
        request_method: Some("POST"),
        request_payload: Some(&payload),
        response_status: Some(StatusCode::OK.as_u16() as i32),
        response_payload: Some(&pos_payload),
        error_message: None,
    };
    let _ = insert_delivery_log(db, log).await;

    Ok(StatusCode::OK)
}

fn normalize_order(
    provider: &str,
    payload: &Value,
    org_id: Uuid,
    store_id: Uuid,
) -> Result<DeliveryOrderNormalized, String> {
    // Provider-specific and generic field mapping for order id.
    let external_order_id = match provider {
        "uber_eats" => payload
            .get("meta")
            .and_then(|m| m.get("resource_id"))
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("order_id").and_then(|v| v.as_str()))
            .or_else(|| payload.get("id").and_then(|v| v.as_str())),
        _ => payload
            .get("order_id")
            .or_else(|| payload.get("id"))
            .and_then(|v| v.as_str()),
    }
    .ok_or_else(|| "missing order id".to_string())?
    .to_string();

    let status = DeliveryOrderStatus::Pending;
    let customer = payload.get("customer").and_then(|c| {
        Some(domain::DeliveryCustomer {
            name: c.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()),
            phone: c.get("phone").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    });
    let delivery_address = payload.get("delivery_address").and_then(|a| {
        Some(domain::DeliveryAddress {
            line1: a.get("line1").and_then(|v| v.as_str()).map(|s| s.to_string()),
            line2: a.get("line2").and_then(|v| v.as_str()).map(|s| s.to_string()),
            city: a.get("city").and_then(|v| v.as_str()).map(|s| s.to_string()),
            postcode: a.get("postcode").and_then(|v| v.as_str()).map(|s| s.to_string()),
            country: a.get("country").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    });
    let items_val = payload.get("items").cloned().unwrap_or_else(|| Value::Array(vec![]));
    let items_array = items_val.as_array().cloned().unwrap_or_default();
    let mut items = Vec::new();
    for it in items_array {
        let name = it
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Item")
            .to_string();
        let quantity = it.get("quantity").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
        let unit_price = it
            .get("unit_price")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| it.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0));
        items.push(domain::DeliveryItem {
            name,
            quantity,
            unit_price,
        });
    }
    let total = payload
        .get("total")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let notes = payload
        .get("notes")
        .or_else(|| payload.get("comment"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(DeliveryOrderNormalized {
        r#type: "delivery_order".to_string(),
        provider: provider.to_string(),
        store_id,
        business_id: org_id,
        external_order_id,
        status,
        customer,
        delivery_address,
        items,
        total,
        notes,
        received_at: Some(Utc::now()),
    })
}

fn verify_traqr_webhook_secret(secret: &str, body: &[u8], provided_sig: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let expected = mac.finalize().into_bytes();
    let expected_hex = hex::encode(expected);
    constant_time_eq_hex(&expected_hex, provided_sig)
}

/// Deliveroo: HMAC-SHA256(webhook_secret, sequence_guid + " " + raw_body), hex.
/// Headers: X-Deliveroo-Sequence-Guid, X-Deliveroo-Hmac-Sha256.
fn verify_deliveroo_webhook(secret: &str, sequence_guid: &str, body: &[u8], provided_sig: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(sequence_guid.as_bytes());
    mac.update(b" ");
    mac.update(body);
    let expected = mac.finalize().into_bytes();
    let expected_hex = hex::encode(expected);
    constant_time_eq_hex(&expected_hex, provided_sig)
}

/// Uber Eats: HMAC-SHA256(client_secret, raw_body), lowercase hex in X-Uber-Signature.
fn verify_uber_eats_webhook(client_secret: &str, body: &[u8], provided_sig: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(client_secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let expected = mac.finalize().into_bytes();
    let expected_hex = hex::encode(expected).to_lowercase();
    let provided_lower = provided_sig.trim().to_lowercase();
    constant_time_eq_hex(&expected_hex, &provided_lower)
}

fn constant_time_eq_hex(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn list_store_delivery_orders(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StorePathParams { store_id }): Path<StorePathParams>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = state
        .db
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "database not available".to_string()))?;
    let store_uuid =
        Uuid::parse_str(&store_id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let since = params.get("since").and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
    let since_utc = since.map(|dt| dt.with_timezone(&Utc));

    let rows = list_delivery_orders_for_store_since(db, &store_uuid.to_string(), since_utc)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let orders: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "id": row.id,
                "org_id": row.org_id,
                "store_id": row.store_id,
                "provider": row.provider,
                "provider_order_id": row.provider_order_id,
                "status": row.status,
                "customer_name": row.customer_name,
                "customer_phone": row.customer_phone,
                "delivery_address": row.delivery_address,
                "items": row.items,
                "total_cents": row.total_cents,
                "notes": row.notes,
                "received_at": row.received_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "orders": orders })))
}

fn internal<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

/// Extract the provider store/restaurant reference from the payload in a provider-specific way.
fn extract_provider_store_reference<'a>(provider: &str, payload: &'a Value) -> Option<&'a str> {
    match provider {
        "deliveroo" => payload.get("location_id").and_then(|v| v.as_str()),
        "uber_eats" => payload
            .get("meta")
            .and_then(|m| m.get("user_id"))
            .and_then(|v| v.as_str()),
        // Just Eat and others: fall back to a generic restaurant_id/store_id field.
        _ => payload
            .get("restaurant_id")
            .or_else(|| payload.get("store_id"))
            .and_then(|v| v.as_str()),
    }
}
