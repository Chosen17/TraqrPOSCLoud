use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::session::CurrentUser;
use crate::state::AppState;
use db::{grant_entitlement, suspend_cloud_sync};

#[derive(Debug, Serialize)]
struct CheckoutSessionResponse {
    checkout_url: String,
}

#[derive(Debug, Deserialize)]
struct CreateCheckoutSessionRequest {}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/billing/create-checkout-session",
            post(create_checkout_session),
        )
        .route("/billing/stripe/webhook", post(stripe_webhook))
}

async fn create_checkout_session(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(_body): Json<CreateCheckoutSessionRequest>,
) -> Result<Json<CheckoutSessionResponse>, (StatusCode, Json<Value>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "message": "database not available" })),
        )
    })?;

    // Look up the user's primary organization (first active membership).
    let row: Option<(String,)> = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT o.id
        FROM organizations o
        JOIN org_memberships om ON om.org_id = o.id
        WHERE om.user_id = ? AND om.status = 'active'
        ORDER BY o.created_at ASC
        LIMIT 1
        "#,
    )
    .bind(&user.0)
    .fetch_optional(db)
    .await
    .map_err(internal)?;

    let (org_id,) = row.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "No organization found for this user" })),
        )
    })?;

    // Fetch user email for Stripe customer_email.
    let user_row: Option<(String,)> =
        sqlx::query_as::<_, (String,)>("SELECT email FROM cloud_users WHERE id = ?")
            .bind(&user.0)
            .fetch_optional(db)
            .await
            .map_err(internal)?;
    let (email,) = user_row.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "User record not found" })),
        )
    })?;

    let stripe_secret =
        std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "Stripe secret key not configured" })),
            )
        })?;
    let price_id =
        std::env::var("STRIPE_PRICE_CLOUD_MONTHLY").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "Stripe price ID not configured" })),
            )
        })?;
    let public_base =
        std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "https://example.com".to_string());

    let success_url = format!("{}/dashboard?billing=success", public_base);
    let cancel_url = format!("{}/pricing?billing=cancelled", public_base);

    // Create a Stripe Checkout Session for subscription.
    let client = reqwest::Client::new();
    let form: Vec<(String, String)> = vec![
        ("mode".to_string(), "subscription".to_string()),
        ("success_url".to_string(), success_url),
        ("cancel_url".to_string(), cancel_url),
        ("customer_email".to_string(), email),
        ("line_items[0][price]".to_string(), price_id),
        ("line_items[0][quantity]".to_string(), "1".to_string()),
        ("metadata[org_id]".to_string(), org_id.clone()),
        ("metadata[user_id]".to_string(), user.0.clone()),
        (
            "subscription_data[metadata][org_id]".to_string(),
            org_id.clone(),
        ),
        (
            "subscription_data[metadata][user_id]".to_string(),
            user.0.clone(),
        ),
    ];

    let res = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .bearer_auth(stripe_secret)
        .form(&form)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "message": format!("Stripe error: {}", e) })),
            )
        })?;

    if !res.status().is_success() {
        let text: String = res.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({ "message": "Stripe checkout failed", "detail": text })),
        ));
    }

    let body: Value = res.json::<Value>().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "message": format!("Invalid Stripe response: {}", e) })),
        )
    })?;

    let url = body
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "message": "Stripe response missing checkout URL" })),
            )
        })?;

    Ok(Json(CheckoutSessionResponse {
        checkout_url: url.to_string(),
    }))
}

fn internal<E: std::fmt::Display>(err: E) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "message": err.to_string() })),
    )
}

async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "message": "database not available" })),
        )
    })?;

    let sig_header = headers
        .get("Stripe-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "message": "Missing Stripe-Signature header" })),
            )
        })?;

    let secret =
        std::env::var("STRIPE_WEBHOOK_SECRET").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "Stripe webhook secret not configured" })),
            )
        })?;

    if !verify_stripe_signature(sig_header, &secret, &body) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Invalid Stripe signature" })),
        ));
    }

    let event: Value = serde_json::from_slice(&body).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": format!("Invalid JSON: {}", e) })),
        )
    })?;

    let event_type = event
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let obj = event
        .get("data")
        .and_then(|d| d.get("object"))
        .cloned()
        .unwrap_or(Value::Null);

    match event_type.as_str() {
        "checkout.session.completed" => {
            if let Some(org_id_str) = obj
                .get("metadata")
                .and_then(|m| m.get("org_id"))
                .and_then(|v| v.as_str())
            {
                if let Ok(org_uuid) = Uuid::parse_str(org_id_str) {
                    let _ = grant_entitlement(db, org_uuid, "cloud_sync").await;
                }
            }
        }
        "customer.subscription.deleted" | "customer.subscription.updated" => {
            let status = obj
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if matches!(status, "canceled" | "unpaid" | "incomplete_expired") {
                if let Some(org_id_str) = obj
                    .get("metadata")
                    .and_then(|m| m.get("org_id"))
                    .and_then(|v| v.as_str())
                {
                    if let Ok(org_uuid) = Uuid::parse_str(org_id_str) {
                        let _ = suspend_cloud_sync(db, org_uuid).await;
                    }
                }
            }
        }
        _ => {
            // Ignore other event types.
        }
    }

    Ok(StatusCode::OK)
}

fn verify_stripe_signature(sig_header: &str, secret: &str, body: &[u8]) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut timestamp = None;
    let mut signature = None;
    for part in sig_header.split(',') {
        let mut kv = part.splitn(2, '=');
        let k = kv.next().unwrap_or("").trim();
        let v = kv.next().unwrap_or("").trim();
        match k {
            "t" => timestamp = Some(v.to_string()),
            "v1" => signature = Some(v.to_string()),
            _ => {}
        }
    }
    let (timestamp, signature) = match (timestamp, signature) {
        (Some(t), Some(s)) => (t, s),
        _ => return false,
    };
    let payload = format!("{}.{}", timestamp, String::from_utf8_lossy(body));

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(payload.as_bytes());
    let expected = mac.finalize().into_bytes();
    let expected_hex = hex::encode(expected);
    constant_time_eq_hex(&expected_hex, &signature)
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
