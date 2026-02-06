use super::{
    ConnectorError, DeliveryConnector, DeliveryIntegrationConfig, TestResult,
    WebhookVerificationStrategy,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{DeliveryAddress, DeliveryCustomer, DeliveryItem, DeliveryOrderNormalized, DeliveryOrderStatus};
use serde::Deserialize;

pub struct UberEatsConnector;

#[async_trait]
impl DeliveryConnector for UberEatsConnector {
    async fn test_connection(
        &self,
        config: &DeliveryIntegrationConfig,
    ) -> Result<TestResult, ConnectorError> {
        // For Uber Eats, a "real" connection test means:
        // - We can obtain an OAuth access token using the provided client credentials.
        // This does a live call to Uber's OAuth endpoint; if credentials are invalid,
        // we surface the error back to the caller.
        let client_id = config
            .client_id
            .as_deref()
            .ok_or_else(|| ConnectorError::InvalidConfig("Uber Eats client_id is required".to_string()))?;
        let client_secret = config
            .client_secret
            .as_deref()
            .ok_or_else(|| ConnectorError::InvalidConfig("Uber Eats client_secret is required".to_string()))?;

        let _token = get_uber_eats_token(client_id, client_secret).await?;

        Ok(TestResult {
            ok: true,
            message: "Successfully obtained Uber Eats access token".to_string(),
        })
    }

    async fn register_webhook(
        &self,
        _config: &DeliveryIntegrationConfig,
        _callback_url: &str,
    ) -> Result<(), ConnectorError> {
        // Webhook URL is configured in Uber Developer Dashboard (Webhooks > Primary Webhook URL).
        Ok(())
    }

    fn webhook_verification_strategy(&self) -> WebhookVerificationStrategy {
        WebhookVerificationStrategy::UberEatsHmacSha256Hex
    }
}

/// Uber Eats webhook notification payload (minimal; full order is fetched via resource_href).
#[derive(Debug, Deserialize)]
pub struct UberEatsWebhookPayload {
    pub event_type: Option<String>,
    pub event_id: Option<String>,
    pub event_time: Option<i64>,
    pub meta: Option<UberEatsWebhookMeta>,
    pub resource_href: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UberEatsWebhookMeta {
    pub resource_id: Option<String>,
    #[serde(rename = "user_id")]
    pub store_id: Option<String>,
    pub status: Option<String>,
}

/// Fetch full order from Uber Eats via resource_href using OAuth client credentials.
/// Requires integration to have client_id and client_secret (Uber Eats app credentials).
pub async fn fetch_uber_eats_order_full(
    client_id: &str,
    client_secret: &str,
    resource_href: &str,
    store_id: uuid::Uuid,
    business_id: uuid::Uuid,
) -> Result<DeliveryOrderNormalized, ConnectorError> {
    let token = get_uber_eats_token(client_id, client_secret).await?;
    let order: UberEatsOrderResponse = reqwest::Client::new()
        .get(resource_href)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept-Encoding", "gzip")
        .send()
        .await
        .map_err(|e| ConnectorError::Http(e.to_string()))?
        .error_for_status()
        .map_err(|e| ConnectorError::Http(e.to_string()))?
        .json()
        .await
        .map_err(|e| ConnectorError::Http(e.to_string()))?;

    normalize_uber_eats_order(order, store_id, business_id)
}

async fn get_uber_eats_token(client_id: &str, client_secret: &str) -> Result<String, ConnectorError> {
    let form = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "client_credentials"),
    ];
    let token_response: UberEatsTokenResponse = reqwest::Client::new()
        .post("https://login.uber.com/oauth/v2/token")
        .form(&form)
        .send()
        .await
        .map_err(|e| ConnectorError::Http(e.to_string()))?
        .error_for_status()
        .map_err(|e| ConnectorError::Auth(e.to_string()))?
        .json()
        .await
        .map_err(|e| ConnectorError::Http(e.to_string()))?;

    token_response
        .access_token
        .ok_or_else(|| ConnectorError::Auth("no access_token in response".to_string()))
}

#[derive(Debug, Deserialize)]
struct UberEatsTokenResponse {
    access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UberEatsOrderResponse {
    id: Option<String>,
    display_id: Option<String>,
    current_state: Option<String>,
    r#type: Option<String>,
    store: Option<UberEatsStore>,
    eater: Option<UberEatsEater>,
    cart: Option<UberEatsCart>,
    payment: Option<UberEatsPayment>,
    placed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UberEatsStore {
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UberEatsEater {
    first_name: Option<String>,
    last_name: Option<String>,
    phone: Option<String>,
    phone_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UberEatsCart {
    items: Option<Vec<UberEatsCartItem>>,
    special_instructions: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UberEatsCartItem {
    id: Option<String>,
    title: Option<String>,
    quantity: Option<i32>,
    price: Option<UberEatsItemPrice>,
    special_instructions: Option<String>,
    selected_modifier_groups: Option<Vec<UberEatsModifierGroup>>,
}

#[derive(Debug, Deserialize)]
struct UberEatsItemPrice {
    unit_price: Option<UberEatsMoney>,
    total_price: Option<UberEatsMoney>,
}

#[derive(Debug, Deserialize)]
struct UberEatsMoney {
    amount: Option<i64>,
    currency_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UberEatsModifierGroup {
    title: Option<String>,
    selected_items: Option<Vec<UberEatsCartItem>>,
}

#[derive(Debug, Deserialize)]
struct UberEatsPayment {
    charges: Option<UberEatsCharges>,
}

#[derive(Debug, Deserialize)]
struct UberEatsCharges {
    total: Option<UberEatsMoney>,
    sub_total: Option<UberEatsMoney>,
    tax: Option<UberEatsMoney>,
}

fn normalize_uber_eats_order(
    order: UberEatsOrderResponse,
    store_id: uuid::Uuid,
    business_id: uuid::Uuid,
) -> Result<DeliveryOrderNormalized, ConnectorError> {
    let external_order_id = order
        .id
        .or_else(|| order.display_id.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let status = match order.current_state.as_deref() {
        Some("CREATED") => DeliveryOrderStatus::Pending,
        Some("ACCEPTED") => DeliveryOrderStatus::Accepted,
        Some("DENIED") => DeliveryOrderStatus::Rejected,
        Some("CANCELED") | Some("CANCELLED") => DeliveryOrderStatus::Cancelled,
        Some("FINISHED") => DeliveryOrderStatus::Delivered,
        _ => DeliveryOrderStatus::Pending,
    };

    let customer = order.eater.as_ref().map(|e| DeliveryCustomer {
        name: e
            .first_name
            .as_ref()
            .zip(e.last_name.as_ref())
            .map(|(a, b)| format!("{} {}", a, b.trim()))
            .or_else(|| e.first_name.clone()),
        phone: e.phone.clone().or(e.phone_code.clone()),
    });

    let delivery_address: Option<DeliveryAddress> = None; // Uber Eats full address often in eater.delivery for merchant delivery

    let mut items = Vec::new();
    if let Some(cart) = &order.cart {
        if let Some(cart_items) = &cart.items {
            for it in cart_items {
                let name = it.title.as_deref().unwrap_or("Item").to_string();
                let quantity = it.quantity.unwrap_or(1);
                let unit_price_cents = it
                    .price
                    .as_ref()
                    .and_then(|p| p.unit_price.as_ref())
                    .and_then(|m| m.amount)
                    .unwrap_or(0);
                let unit_price = (unit_price_cents as f64) / 100.0;
                let mut item_name = name;
                if let Some(groups) = &it.selected_modifier_groups {
                    for g in groups {
                        if let Some(sel) = &g.selected_items {
                            for s in sel {
                                if let Some(t) = &s.title {
                                    item_name.push_str(" + ");
                                    item_name.push_str(t);
                                }
                            }
                        }
                    }
                }
                items.push(DeliveryItem {
                    name: item_name,
                    quantity,
                    unit_price,
                });
            }
        }
    }

    let total_cents = order
        .payment
        .as_ref()
        .and_then(|p| p.charges.as_ref())
        .and_then(|c| c.total.as_ref())
        .and_then(|m| m.amount)
        .unwrap_or(0);
    let total = (total_cents as f64) / 100.0;

    let notes = order
        .cart
        .as_ref()
        .and_then(|c| c.special_instructions.clone());

    let received_at = order
        .placed_at
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(DeliveryOrderNormalized {
        r#type: "delivery_order".to_string(),
        provider: "uber_eats".to_string(),
        store_id,
        business_id,
        external_order_id,
        status,
        customer,
        delivery_address,
        items,
        total,
        notes,
        received_at: received_at.or_else(|| Some(Utc::now())),
    })
}
