## Delivery Integrations – Just Eat, Deliveroo, Uber Eats

This document describes how delivery platform integrations work in Traqr Cloud.

### Tenancy and Store Mapping

- **Business** in this context maps to an `organization` (`organizations.id`).
- Each **Store** is a row in `stores` with `org_id` referencing the owning organization.
- Delivery integrations are **store-scoped**:
  - `delivery_integrations.org_id` = business/organization.
  - `delivery_integrations.store_id` = specific store.
  - `UNIQUE(store_id, provider)` ensures one configuration per provider per store.
- Incoming webhooks are mapped to the correct store by:
  - Provider restaurant/store ID in the webhook payload (e.g. `restaurant_id`, `store_id`).
  - This value is stored as `delivery_integrations.provider_store_reference`.
  - The webhook handler looks up `delivery_integrations` by `(provider, provider_store_reference)` to determine `org_id` and `store_id`.

### Database Tables

#### `delivery_integrations`

- Stores per-store configuration and status for each provider.
- Key columns:
  - `id CHAR(36)` – UUID primary key.
  - `org_id CHAR(36)` – owning organization (business).
  - `store_id CHAR(36)` – store this integration applies to.
  - `provider VARCHAR(50)` – `just_eat`, `deliveroo`, `uber_eats`.
  - `status VARCHAR(50)` – `disconnected`, `pending`, `connected`, `error`.
  - Encrypted credentials:
    - `api_key_enc`, `client_id_enc`, `client_secret_enc`,
      `access_token_enc`, `refresh_token_enc`, `webhook_secret_enc`.
  - `provider_store_reference VARCHAR(255)` – provider’s restaurant/store identifier.
  - `last_sync_at DATETIME(3)` – last successful sync/webhook.
  - `last_error_message TEXT` – most recent error, if any.
- Constraints:
  - `UNIQUE (store_id, provider)` – per-store, per-provider configuration.
  - `FOREIGN KEY (org_id)` → `organizations(id)`.
  - `FOREIGN KEY (store_id)` → `stores(id)`.

#### `delivery_orders`

- Normalized orders from delivery platforms.
- Key columns:
  - `id CHAR(36)` – UUID primary key.
  - `org_id CHAR(36)` – business/organization.
  - `store_id CHAR(36)` – target store.
  - `integration_id CHAR(36)` – FK to `delivery_integrations.id`.
  - `provider VARCHAR(50)` – `just_eat`, `deliveroo`, `uber_eats`.
  - `provider_order_id VARCHAR(255)` – provider’s order identifier.
  - `status VARCHAR(50)` – `pending`, `accepted`, `rejected`, `cancelled`, `ready`, `collected`, `delivered`.
  - `customer_name`, `customer_phone`.
  - `delivery_address JSON` – normalized address.
  - `items JSON` – array of normalized line items.
  - Monetary fields in cents: `subtotal_cents`, `tax_cents`, `delivery_fee_cents`, `total_cents`.
  - `notes TEXT` – free-form notes/instructions.
  - `raw_payload JSON` – original provider payload.
  - `received_at DATETIME(3)` – when Cloud received the order.
- Constraints/indexes:
  - `UNIQUE (provider, provider_order_id)` – idempotent inserts/upserts per provider.
  - `INDEX (store_id, received_at)` – listing orders per store.
  - `INDEX (integration_id, received_at)` – listing per integration.

#### `delivery_integration_logs`

- Intended for auditing inbound/outbound provider calls.
- Columns:
  - `id BIGINT AUTO_INCREMENT` – primary key.
  - `provider`, `store_id`, `integration_id`.
  - `request_url`, `request_method`.
  - `request_payload JSON`, `response_status INT`, `response_payload JSON`.
  - `error_message TEXT`.
  - `created_at DATETIME(3)` – timestamp.
- Helper in `db::delivery_integrations` (`insert_delivery_log`) can be used to record connector and webhook interactions.

### Credential Storage and Encryption

- Sensitive fields in `delivery_integrations` (API keys, client secrets, tokens, webhook secrets) are stored **encrypted at rest**:
  - Columns use `_enc` suffix (e.g. `api_key_enc`), holding base64-encoded ciphertext.
- Encryption is implemented in `crates/cloud_api/src/crypto.rs`:
  - Uses **AES-256-GCM** with a random 12-byte nonce.
  - Ciphertext format: `base64(nonce || ciphertext)`.
  - Key is provided via environment variable `DELIVERY_CRED_ENC_KEY`:
    - Must be a base64-encoded 32-byte value (256-bit key).
  - Functions:
    - `encrypt_secret(plaintext: &str) -> Result<String, String>`.
    - `decrypt_secret(ciphertext_b64: &str) -> Result<String, String>`.
- The application:
  - Encrypts secrets before writing to the database.
  - Decrypts them only in memory when calling provider APIs.
  - Never returns decrypted secrets from any API.

### Webhook Endpoints and Verification

- Generic webhooks per provider (under `/api/webhooks/*` in the main app):
  - `POST /api/webhooks/just_eat`
  - `POST /api/webhooks/deliveroo`
  - `POST /api/webhooks/uber_eats`
- Implementation lives in `crates/cloud_api/src/routes/delivery_webhooks.rs`:
  - Entry handlers:
    - `handle_just_eat_webhook`
    - `handle_deliveroo_webhook`
    - `handle_uber_eats_webhook`
  - All delegate to `handle_provider_webhook(app_state, provider, payload)`.
- Webhook verification:
  - The current implementation assumes webhook authenticity is enforced by provider configuration (secret registration is stubbed).
  - `register_webhook` in each connector is the place to:
    - Configure provider-side secret or signing key.
    - Persist `webhook_secret_enc` on success.
  - `handle_provider_webhook` can be extended to:
    - Read request headers.
    - Recompute HMAC/signature using decrypted `webhook_secret`.
    - Reject (`401`) if signature mismatch.

### Order Normalization and Internal Format

- Webhook payloads are normalized via:
  - `normalize_order(provider, &payload, org_id, store_id) -> DeliveryOrderNormalized`.
  - Implemented in `delivery_webhooks.rs` using domain types from `crates/domain`:
    - `DeliveryOrderNormalized`
    - `DeliveryOrderStatus`
    - `DeliveryCustomer`
    - `DeliveryAddress`
    - `DeliveryItem`
- `DeliveryOrderNormalized` matches the **POS-facing universal payload**:

```json
{
  "type": "delivery_order",
  "provider": "just_eat",
  "store_id": "UUID",
  "business_id": "UUID",
  "external_order_id": "JE-919191",
  "status": "pending",
  "customer": {
    "name": "John Smith",
    "phone": "+447700900123"
  },
  "delivery_address": {
    "line1": "10 High Street",
    "postcode": "SW1A 1AA"
  },
  "items": [
    {
      "name": "Chicken Burger",
      "quantity": 2,
      "unit_price": 7.99
    }
  ],
  "total": 15.98,
  "notes": "No onions"
}
```

- Normalization rules (initial generic mapping):
  - `external_order_id`:
    - Taken from `payload.order_id` or `payload.id`.
  - `status`:
    - Currently defaults to `pending`.
    - Should be extended to map provider-specific states into the `DeliveryOrderStatus` enum.
  - `customer`:
    - From `payload.customer.{name,phone}` when present.
  - `delivery_address`:
    - From `payload.delivery_address.{line1,line2,city,postcode,country}`.
  - `items`:
    - From `payload.items` array; each element mapped to `DeliveryItem`:
      - `name` from `item.name` (fallback `"Item"`).
      - `quantity` from `item.quantity` (default `1`).
      - `unit_price` from `item.unit_price` or `item.price`.
  - `total`:
    - From `payload.total` (float).
  - `notes`:
    - From `payload.notes` or `payload.comment`.

### Webhook → Database → POS Flow

1. **Incoming webhook**:
   - Provider POSTs JSON payload to `/api/webhooks/{provider}`.
   - Handler parses JSON and identifies the provider store/restaurant ID:
     - `payload.restaurant_id` or `payload.store_id`.
2. **Store mapping**:
   - Cloud looks up `delivery_integrations` by:
     - `provider` (e.g. `just_eat`).
     - `provider_store_reference` (provider’s restaurant ID).
   - If not found, the webhook is rejected with 400 and an error is logged.
3. **Normalization**:
   - `normalize_order` builds a `DeliveryOrderNormalized` instance with:
     - `store_id` and `business_id` (org_id) from the integration row.
     - Provider, external order ID, status, customer, address, items, totals, notes.
4. **Persist normalized order**:
   - `insert_delivery_order` writes or upserts into `delivery_orders`:
     - Uses `UNIQUE(provider, provider_order_id)` to ensure idempotency.
     - `raw_payload` contains the full original JSON payload.
5. **Emit POS command**:
   - The same normalized struct is serialized into JSON (`pos_order_payload`).
   - `db::enqueue_delivery_order_command` enqueues a `delivery_order` command to `device_command_queue`:
     - Chooses device:
       - Prefer `stores.canonical_device_id`.
       - Fallback: most recently updated device from `device_sync_state` for the store.
     - Inserts row with:
       - `command_type = 'delivery_order'`.
       - `command_body` = normalized POS payload.
       - `status = 'queued'`, `sensitive = 0`.
6. **POS pulls commands**:
   - POS devices continue to poll `/api/sync/commands` as today.
   - `delivery_order` commands appear alongside other commands.
   - POS decodes `command_body` according to the universal `pos_order_payload` schema.

### Admin UI and Store Flow

- In the **Store admin** page (`web/public/store.html`):
  - A **Delivery Integrations** tab is added:
    - Buttons: `Devices`, `Menu`, `Orders`, `Command Center`, `Delivery Integrations`.
  - Within `Delivery Integrations`:
    - Three cards (Just Eat, Deliveroo, Uber Eats) are rendered by JS `loadDeliveryIntegrations(storeId)`:
      - Show:
        - Provider name.
        - Status (Connected/Pending/Disconnected/Error) with colour badge.
        - Last sync time.
        - Last error message (if any).
      - Actions:
        - **Connect/Disconnect**
        - **Test connection**
        - **View logs** (stub – navigates to ops page, to be expanded).
- Backing APIs in `delivery_webhooks.rs`:
  - `GET /api/portal/stores/:store_id/delivery_integrations`
    - Returns a map `{ provider_code: { id, org_id, store_id, provider, status, last_sync_at, last_error_message } }`.
  - `POST /api/portal/stores/:store_id/delivery_integrations/:provider/connect`
    - Body (generic shape):
      - Common: `{ "api_key": "...", "provider_store_reference": "..." }`.
      - Uber Eats additionally supports:
        - `client_id` – Uber Eats application client id.
        - `client_secret` – Uber Eats application client secret (stored encrypted; used for OAuth + webhook verification).
    - Flow:
      - Encrypts `api_key` and upserts `delivery_integrations` with `status = 'pending'`.
      - For Uber Eats, also encrypts and stores `client_id` / `client_secret`.
      - Builds in-memory `DeliveryIntegrationConfig`.
      - Calls `DeliveryConnector.test_connection`.
      - On success:
        - Calls `DeliveryConnector.register_webhook` (stub).
        - Marks `status = 'connected'`.
      - On failure:
        - Sets `status = 'error'`, updates `last_error_message`.
  - `POST /api/portal/stores/:store_id/delivery_integrations/:provider/disconnect`
    - Clears credential fields and sets `status = 'disconnected'`.
  - `POST /api/portal/stores/:store_id/delivery_integrations/:provider/test`
    - Decrypts stored credentials and calls `test_connection` for a health check.
- UX guarantees:
  - User only needs to paste credentials and click **Connect**.
  - Webhook URLs and store mapping are completely automated by Cloud.

### Connector Responsibilities and Extensibility

- Connectors live in `crates/cloud_api/src/delivery_connectors/`:
  - `mod.rs` defines:
    - `DeliveryConnector` trait:
      - `test_connection`
      - `register_webhook`
      - `refresh_token_if_needed`
      - `fetch_orders` (fallback if webhooks fail).
    - `DeliveryIntegrationConfig` – decrypted view of credentials and IDs.
    - Factory: `connector_for(DeliveryProvider) -> Box<dyn DeliveryConnector>`.
  - Provider stubs:
    - `JustEatConnector`
    - `DeliverooConnector`
    - `UberEatsConnector`
- To add a new provider:
  1. Extend `DeliveryProvider` enum in `crates/domain`.
  2. Add a new column value constraint (documentation-level) for `delivery_integrations.provider`.
  3. Implement a new connector in `delivery_connectors` and branch in `connector_for`.
  4. Update:
     - Admin UI to render a new card.
     - `normalize_order` to handle the provider’s payload shape.
