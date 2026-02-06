# Delivery Provider API References (Incoming Orders)

This document compiles **official documentation** for receiving incoming orders from Just Eat, Deliveroo, and Uber Eats. Use it to implement real API calls and webhook verification in the Traqr Cloud delivery connectors.

**Implementation status (as of this doc):**
- **Deliveroo:** Webhook verification using official HMAC (X-Deliveroo-Sequence-Guid + X-Deliveroo-Hmac-Sha256). Order payload normalized from webhook body.
- **Uber Eats:** Webhook verification using X-Uber-Signature (HMAC-SHA256 of body with client_secret). Optional full order fetch via GET resource_href with OAuth client_credentials when `client_id` and `client_secret` are stored (connect body supports optional `client_id` / `client_secret` for Uber Eats).
- **Just Eat:** Traqr internal HMAC or no verification; connector and payload mapping still stubbed until JET Connect payload/signature docs are confirmed.

---

## 1. Just Eat Takeaway.com

### Documentation links
- **Public developer portal:** https://developers.just-eat.com/
- **Documentation overview:** https://developers.just-eat.com/documentation
- **UK API docs (Swagger):** https://uk.api.just-eat.io/docs
- **JET Connect API (orders / dispatch):** https://uk.api.just-eat.io/docs/jetconnect/index.html

### Authentication
- **Partner API:** API key in header `JE-API-KEY`.
- All calls and callbacks require **HTTPS** on port 443 with a valid SSL certificate.

### Webhooks (async)
- Just Eat supports **async webhooks** for long-running operations.
- Your webhook may receive a `?callback={returnUrl}` query parameter.
- **Response:** Return **202 Accepted** immediately to acknowledge receipt.
- Then perform work and POST result to the callback URL:
  - Success: `{"status": "Success", "message": "{successMessage}", "data": {}}`
  - Failure: `{"status": "Failure", "message": "{failureMessage}", "data": {}}`

### Order / JET Connect
- **JET Connect** covers “Async Webhooks” and “JET Connect for Dispatch Orders” (order-related notifications).
- Full webhook payload structure (e.g. `restaurant_id`, order fields) is in the JET Connect docs; the Swagger UI at https://uk.api.just-eat.io/docs/jetconnect/index.html may need to load fully to see it.
- **Integration contact:** david.handley@justeattakeaway.com (for partner access or payload details).

### Implementation notes for Traqr
- **Connect flow:** Call Just Eat API with `JE-API-KEY` (stored encrypted) to verify credentials; implement in `just_eat.rs` `test_connection`.
- **Webhook registration:** If the partner API exposes a “register webhook URL” endpoint, use it in `register_webhook`; otherwise webhook URL may be configured in Just Eat’s partner portal.
- **Webhook handler:** Accept POST, return 202 quickly, then process and optionally call callback URL. If Just Eat sends a **signature header**, verify it per their docs (not yet documented in the public snippets above).
- **Store mapping:** Use the provider’s restaurant/store identifier from the webhook (e.g. `restaurant_id`) as `provider_store_reference` for lookup in `delivery_integrations`.

---

## 2. Deliveroo

### Documentation links
- **Picking API webhook (order status):** https://api-docs.deliveroo.com/docs/picking-api-webhook
- **Securing webhooks (HMAC):** https://api-docs.deliveroo.com/docs/securing-webhooks
- **Configure webhooks (register URL):** https://api-docs.deliveroo.com/reference/configure-webhooks
- **Webhook receiver example:** https://api-docs.deliveroo.com/reference/webhook-receiver-example

### Order status webhook
- **Events:** Order status callback is triggered for:
  - **PLACED** – order created  
  - **ACCEPTED** – partner accepted  
  - **REJECTED** – partner rejected  
  - **CANCELLED** – partner cancelled  
- Callbacks may be **out of order or duplicate**; use idempotent processing (e.g. upsert by `(provider, provider_order_id)`).
- **Configuration:** Webhook URL is set in the **Deliveroo Developer Portal** (Webhooks section). URLs must be **HTTPS**.

### Authentication (webhook verification)
- You get a **webhook secret** from the Developer Portal (Webhooks section) — **different from** `client-secret`.
- **Headers:**
  - `X-Deliveroo-Sequence-Guid` – sequential GUID
  - `X-Deliveroo-Hmac-Sha256` – HMAC signature
- **Signed payload (for non-legacy Picking API webhooks):**  
  `GUID + " " + raw_request_body`  
  (single space between GUID and body; use **raw bytes** of the body, no JSON re-serialisation.)
- **Legacy POS webhooks** (`new_order` / `cancel_order`): use `GUID + " \n " + raw_body` (newline with spaces).
- **Compute:** `HMAC-SHA256(webhook_secret, signed_payload)` and compare **hex** with `X-Deliveroo-Hmac-Sha256`.

### Registering webhooks
- **Base URL:** `https://api.developers.deliveroo.com/signature/v1/webhooks`
- Configure and manage webhook endpoints via the Developer Portal and/or the “Register webhooks” API reference.

### Response and retries
- Return **HTTP 2xx** for success; otherwise Deliveroo retries with exponential backoff and circuit breaking.
- For order status, Deliveroo stops retrying after **30 minutes**.

### Implementation notes for Traqr
- **Verification:** In `deliveroo.rs` (or shared webhook handler), use **Deliveroo’s** verification: read `X-Deliveroo-Sequence-Guid` and `X-Deliveroo-Hmac-Sha256`, then `HMAC-SHA256(webhook_secret, guid + " " + raw_body)` (hex). Do **not** use Traqr’s internal HMAC for Deliveroo; add a `WebhookVerificationStrategy::DeliverooHmacSha256` (or provider-specific enum) and verify accordingly.
- **Store mapping:** Payload uses `location_id` for store; our `extract_provider_store_reference` for Deliveroo already uses `location_id`.
- **Payload:** Use the webhook receiver example reference for the exact JSON shape; normalize into `DeliveryOrderNormalized` in the webhook handler.

---

## 3. Uber Eats

### Documentation links
- **Webhooks guide:** https://developer.uber.com/docs/eats/guides/webhooks
- **Order notification webhook:** https://developer.uber.com/docs/eats/references/api/webhooks.orders-notification
- **Get Order (fetch full order):** https://developer.uber.com/docs/eats/references/api/v2/get-eats-order-orderid
- **Order integration guide:** https://developer.uber.com/docs/eats/guides/order-integration

### Webhook setup
- **Primary Webhook URL** is set in the **Uber Developer Dashboard** → your Application → **Webhooks**.
- All webhook notifications go to this single URL.
- Dashboard supports **Basic Auth** or **OAuth** for the webhook endpoint (optional).

### Order notification (`orders.notification`)
- **Event type:** `orders.notification` when an order is created.
- **Headers:**
  - `X-Environment` – `production` or `sandbox`
  - `X-Uber-Signature` – **HMAC-SHA256** of the **raw request body**, key = **client secret**, value = **lowercase hex**.
- **Verification:**  
  `expected_hex = HMAC-SHA256(client_secret, raw_body).hexdigest().lower()`  
  Compare with `X-Uber-Signature`.

### Webhook payload (minimal; you then fetch full order)
- `event_type` – e.g. `"orders.notification"`
- `event_id` – unique event id (dedupe)
- `event_time` – Unix timestamp
- `meta.user_id` – **store/location id** (use for `provider_store_reference` lookup)
- `meta.resource_id` – **order id**
- `meta.status` – e.g. `"pos"`
- `resource_href` – full URL to **GET** the order details (e.g. `https://api.uber.com/v2/eats/order/{order_id}`)

### Expected response
- Return **HTTP 200** with **empty body**. Any other status or timeout triggers retries (exponential backoff, up to 7 attempts).

### After receiving the webhook
1. Acknowledge with **200** immediately.
2. **Fetch full order:** `GET resource_href` (or `GET https://api.uber.com/v2/eats/order/{order_id}`) with **OAuth 2.0 Bearer** token (scopes: `eats.order` or `eats.store.orders.read`).
3. **Accept or deny** within **11.5 minutes** via:
   - **POST** `/accept_pos_order`
   - **POST** `/deny_pos_order`  
   Otherwise the order can auto-cancel; robocalls may trigger after 90 seconds if configured.

### Get Order response (summary)
- **id**, **display_id**, **current_state**, **type** (e.g. PICK_UP, DELIVERY_BY_UBER), **brand** (UBER_EATS, POSTMATES).
- **store** – id, name, integrator_store_id, merchant_store_id.
- **eater** – first_name, last_name, phone, phone_code.
- **cart** – items (id, title, quantity, price, selected_modifier_groups, special_instructions), special_instructions (order-level).
- **payment.charges** – total, sub_total, tax, total_fee, etc. (amounts in smallest currency unit).
- **placed_at**, **estimated_ready_for_pickup_at**, **deliveries**, etc.

### Implementation notes for Traqr
- **Verification:** Use **Uber’s** scheme: `X-Uber-Signature` = HMAC-SHA256(**client_secret**, raw_body) in **lowercase hex**. Store Uber **client_secret** encrypted (e.g. in `client_secret_enc` or a dedicated field) and use it only for webhook verification and API calls. Add a strategy e.g. `WebhookVerificationStrategy::UberEatsHmacSha256Hex`.
- **Store mapping:** `meta.user_id` = store id; our `extract_provider_store_reference` for Uber Eats currently uses `meta.user_id` — correct.
- **Full order:** After verifying the webhook, call **GET** `resource_href` with OAuth Bearer token to get cart, eater, payment, etc.; then normalize into `DeliveryOrderNormalized` and persist.
- **Accept/Deny:** To avoid auto-cancel, the POS or Cloud must call Uber’s accept/deny endpoints within 11.5 minutes (can be a separate flow once the order is on the POS).

---

## Summary table

| Provider    | Webhook verification                    | Store ID in payload     | Full order data                    | Docs / config                          |
|------------|------------------------------------------|--------------------------|------------------------------------|----------------------------------------|
| **Just Eat** | Partner-specific (check JET Connect docs) | e.g. `restaurant_id`     | In webhook or callback             | JET Connect + partner contact          |
| **Deliveroo** | HMAC-SHA256(secret, `GUID + " " + raw_body`), header `X-Deliveroo-Hmac-Sha256` | `location_id`            | In webhook payload                 | Developer Portal + securing-webhooks   |
| **Uber Eats** | HMAC-SHA256(client_secret, raw_body) hex, header `X-Uber-Signature` | `meta.user_id`           | GET `resource_href` (OAuth Bearer)  | Developer Dashboard + webhooks + Get Order |

---

## Next steps in code

1. **Just Eat** (`just_eat.rs`): Add real `test_connection` (e.g. call a Just Eat API with `JE-API-KEY`); implement `register_webhook` if API or portal allows; in webhook handler, return 202 and process; add signature verification if documented by Just Eat.
2. **Deliveroo** (`deliveroo.rs`): Add Deliveroo HMAC verification (GUID + space + raw body) in the webhook path; optionally add `test_connection` against Deliveroo API; webhook URL configured in portal.
3. **Uber Eats** (`uber_eats.rs`): Add Uber signature verification (`X-Uber-Signature`, client_secret, raw body); after verification, GET order by `resource_href` with OAuth; normalize Get Order response to `DeliveryOrderNormalized`; optionally add accept/deny calls within 11.5 minutes.

Use this doc together with `docs/delivery_integrations.md` and `docs/DELIVERY_INTEGRATIONS_QA_REPORT.md` for end-to-end behaviour and QA.
