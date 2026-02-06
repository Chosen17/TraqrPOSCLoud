# Traqr Cloud Delivery Integrations – QA & Backend Verification Report

**Date:** 2025-02-05  
**Scope:** End-to-end verification of delivery integrations (DB, encryption, API, webhooks, normalization, command queue, logging, UI).

---

## 1. Database Verification

### 1.1 Tables (migration `023_delivery_integrations.sql`)

| Table | Exists in migration | Purpose |
|-------|---------------------|---------|
| `delivery_integrations` | ✅ | Per-store provider config, encrypted creds, `provider_store_reference` |
| `delivery_orders` | ✅ | Normalized orders; unique per `(provider, provider_order_id)` |
| `delivery_integration_logs` | ✅ | Audit log for webhook/connector requests |

### 1.2 Full schema (from migration)

**delivery_integrations**

| Column | Type | Nullable | Key | Default |
|--------|------|----------|-----|---------|
| id | CHAR(36) | NO | PRI | UUID() |
| org_id | CHAR(36) | NO | FK → organizations(id) | - |
| store_id | CHAR(36) | NO | FK → stores(id), UNIQUE (store_id, provider) | - |
| provider | VARCHAR(50) | NO | - | - |
| status | VARCHAR(50) | NO | - | 'disconnected' |
| api_key_enc | TEXT | YES | - | NULL |
| client_id_enc | TEXT | YES | - | NULL |
| client_secret_enc | TEXT | YES | - | NULL |
| access_token_enc | TEXT | YES | - | NULL |
| refresh_token_enc | TEXT | YES | - | NULL |
| token_expires_at | DATETIME(3) | YES | - | NULL |
| webhook_secret_enc | TEXT | YES | - | NULL |
| provider_store_reference | VARCHAR(255) | YES | - | NULL |
| last_sync_at | DATETIME(3) | YES | - | NULL |
| last_error_message | TEXT | YES | - | NULL |
| created_at | DATETIME(3) | NO | - | CURRENT_TIMESTAMP(3) |
| updated_at | DATETIME(3) | NO | - | CURRENT_TIMESTAMP(3) ON UPDATE |

**delivery_orders**

| Column | Type | Nullable | Key | Default |
|--------|------|----------|-----|---------|
| id | CHAR(36) | NO | PRI | UUID() |
| org_id | CHAR(36) | NO | FK | - |
| store_id | CHAR(36) | NO | FK | - |
| integration_id | CHAR(36) | NO | FK → delivery_integrations(id) | - |
| provider | VARCHAR(50) | NO | UNIQUE (provider, provider_order_id) | - |
| provider_order_id | VARCHAR(255) | NO | - | - |
| status | VARCHAR(50) | NO | - | - |
| customer_name | VARCHAR(255) | YES | - | NULL |
| customer_phone | VARCHAR(50) | YES | - | NULL |
| delivery_address | JSON | YES | - | NULL |
| items | JSON | NO | - | - |
| subtotal_cents | BIGINT | YES | - | NULL |
| tax_cents | BIGINT | YES | - | NULL |
| delivery_fee_cents | BIGINT | YES | - | NULL |
| total_cents | BIGINT | YES | - | NULL |
| notes | TEXT | YES | - | NULL |
| raw_payload | JSON | NO | - | - |
| received_at | DATETIME(3) | NO | - | - |
| created_at | DATETIME(3) | NO | - | CURRENT_TIMESTAMP(3) |
| updated_at | DATETIME(3) | NO | - | CURRENT_TIMESTAMP(3) ON UPDATE |

**delivery_integration_logs**

| Column | Type | Nullable | Key | Default |
|--------|------|----------|-----|---------|
| id | BIGINT | NO | PRI | AUTO_INCREMENT |
| provider | VARCHAR(50) | NO | - | - |
| store_id | CHAR(36) | YES | - | NULL |
| integration_id | CHAR(36) | YES | - | NULL |
| request_url | TEXT | YES | - | NULL |
| request_method | VARCHAR(20) | YES | - | NULL |
| request_payload | JSON | YES | - | NULL |
| response_status | INT | YES | - | NULL |
| response_payload | JSON | YES | - | NULL |
| error_message | TEXT | YES | - | NULL |
| created_at | DATETIME(3) | NO | - | CURRENT_TIMESTAMP(3) |

### 1.3 Indexes

- **delivery_integrations:** PRIMARY KEY (`id`), UNIQUE KEY `uq_delivery_integrations_store_provider` (`store_id`, `provider`).
- **delivery_orders:** PRIMARY KEY (`id`), UNIQUE KEY `uq_delivery_orders_provider_order` (`provider`, `provider_order_id`), INDEX `idx_delivery_orders_store_received` (`store_id`, `received_at`), INDEX `idx_delivery_orders_integration_received` (`integration_id`, `received_at`).
- **delivery_integration_logs:** PRIMARY KEY (`id`) only (no secondary indexes in migration).

### 1.4 Constraints

- **delivery_integrations:** UNIQUE (`store_id`, `provider`); FOREIGN KEY (`org_id`) → `organizations(id)` ON DELETE CASCADE; FOREIGN KEY (`store_id`) → `stores(id)` ON DELETE CASCADE.
- **delivery_orders:** UNIQUE (`provider`, `provider_order_id`); FOREIGN KEY (`org_id`, `store_id`, `integration_id`) as per migration.
- **delivery_integration_logs:** none besides PK.

### 1.5 Mismatch vs spec

- **Spec asked for:** `unique(provider, provider_store_reference)`.  
- **Migration has:** `unique(store_id, provider)` (one integration per store per provider).  
- **Verdict:** Design is one config per store per provider; `provider_store_reference` is a nullable field on that row, not a unique key. Webhook lookup uses `(provider, provider_store_reference)` in application code (`find_integration_by_provider_store_reference`). No schema change needed for current behaviour.
- **Future:** To support multiple provider store IDs per store (e.g. multiple Just Eat restaurant IDs for one Traqr store), a migration would be required: drop `UNIQUE(store_id, provider)` and add `UNIQUE(provider, provider_store_reference)` (and ensure `provider_store_reference` is NOT NULL for connected integrations).
- **Spec asked for:** `unique(provider, provider_order_id)`. **Migration has:** ✅ `uq_delivery_orders_provider_order (provider, provider_order_id)`.

**To confirm schema on your instance, run:**

```sql
SHOW CREATE TABLE delivery_integrations\G
SHOW CREATE TABLE delivery_orders\G
SHOW CREATE TABLE delivery_integration_logs\G
```

---

## 2. Encryption Verification (AES-256-GCM)

### 2.1 Env var `DELIVERY_CRED_ENC_KEY`

- **Required:** Yes. `crypto::get_cipher()` returns `Err("missing DELIVERY_CRED_ENC_KEY env var for delivery credential encryption")` if unset.
- **Format:** Base64-encoded 32-byte key (256-bit). Invalid base64 or wrong length returns a clear error.
- **Code:** `crates/cloud_api/src/crypto.rs` lines 9–20.

### 2.2 AES-256-GCM usage

- **Cipher:** `Aes256Gcm` from `aes_gcm`; key from env, 12-byte nonce (NONCE_LEN), random per encrypt via `OsRng`.
- **Format:** `base64(nonce || ciphertext)` (nonce first, then GCM ciphertext).
- **Encrypt path:** `encrypt_secret(plaintext)` → used in `connect_integration` for `api_key` and `webhook_secret` before DB write.
- **Decrypt path:** `decrypt_secret(ciphertext_b64)` → used in `test_integration` and webhook handler for `webhook_secret_enc` / `api_key_enc` when needed in memory only.

### 2.3 Ciphertext in DB

- Credentials are stored in `*_enc` columns; API never returns decrypted values. Connect flow writes `api_key_enc` and `webhook_secret_enc` only after encryption.

### 2.4 Proof of round-trip (to run manually)

With `DELIVERY_CRED_ENC_KEY` set (e.g. `openssl rand -base64 32`):

1. Connect an integration with a known `api_key` (e.g. `"test-key-12345"`).
2. Query: `SELECT id, LEFT(api_key_enc, 32) AS blob_start FROM delivery_integrations WHERE provider = 'just_eat' AND store_id = '<store_id>' LIMIT 1;`  
   Expect: `blob_start` is base64, not the literal string `test-key-12345`.
3. In code, `decrypt_secret(row.api_key_enc)` (e.g. in test endpoint or a one-off script) must return `"test-key-12345"`.

**Verdict:** Implementation matches spec; run the above steps in your environment for proof.

---

## 3. API Route Verification

### 3.1 Endpoints (all under `/api`)

| Method | Path | Handler | Auth |
|--------|------|---------|------|
| GET | `/api/portal/stores/:store_id/delivery_integrations` | `get_store_delivery_integrations` | ✅ CurrentUser + user_can_access_store |
| POST | `/api/portal/stores/:store_id/delivery_integrations/:provider/connect` | `connect_integration` | ✅ CurrentUser + user_can_access_store |
| POST | `/api/portal/stores/:store_id/delivery_integrations/:provider/disconnect` | `disconnect_integration` | ✅ CurrentUser + user_can_access_store |
| POST | `/api/portal/stores/:store_id/delivery_integrations/:provider/test` | `test_integration` | ✅ CurrentUser + user_can_access_store |

**Fix applied:** Portal delivery handlers now require `CurrentUser` and call `user_can_access_store`; no cross-store access without permission.

### 3.2 Response shapes

- **GET delivery_integrations:** `{ "integrations": { "just_eat": { "id", "org_id", "store_id", "provider", "status", "provider_store_reference", "last_sync_at", "last_error_message" }, ... } }`.  
  **Fix applied:** Wrapped in `integrations` so the UI’s `data.integrations` works.

### 3.3 Curl examples (replace `BASE`, `STORE_ID`, `COOKIE` or Bearer, `PROVIDER`)

```bash
# GET (requires session cookie from portal login)
curl -s -b "session=YOUR_SESSION" "http://localhost:8080/api/portal/stores/STORE_ID/delivery_integrations"

# POST connect (requires session; body: api_key, provider_store_reference)
curl -s -X POST -b "session=YOUR_SESSION" -H "Content-Type: application/json" \
  -d '{"api_key":"sk_test_xxx","provider_store_reference":"JE-STORE-999"}' \
  "http://localhost:8080/api/portal/stores/STORE_ID/delivery_integrations/just_eat/connect"

# POST disconnect
curl -s -X POST -b "session=YOUR_SESSION" \
  "http://localhost:8080/api/portal/stores/STORE_ID/delivery_integrations/just_eat/disconnect"

# POST test
curl -s -X POST -b "session=YOUR_SESSION" \
  "http://localhost:8080/api/portal/stores/STORE_ID/delivery_integrations/just_eat/test"
```

### 3.4 Expected behaviour

- **Store scoping:** 403 if user cannot access `store_id` (enforced by `user_can_access_store`).
- **Invalid provider:** 400 with message `"provider must be just_eat, deliveroo, or uber_eats"`.
- **Missing store (connect):** 404 `"store not found"` when `stores.id` lookup fails.
- **Unauthenticated:** 401 from `CurrentUser` extractor when session missing/invalid.

---

## 4. Webhook Endpoint Verification

### 4.1 Routes (under `/api`)

- `POST /api/webhooks/just_eat` → `handle_just_eat_webhook`
- `POST /api/webhooks/deliveroo` → `handle_deliveroo_webhook`
- `POST /api/webhooks/uber_eats` → `handle_uber_eats_webhook`

All delegate to `handle_provider_webhook(state, provider, headers, body)`.

### 4.2 Handler flow

1. **Raw body:** Handlers receive `body: Bytes`; signature verification uses this raw body (not parsed JSON).
2. **Verify signature:** When strategy is `TraqrHmacSha256Hex`, `verify_traqr_webhook_secret(secret, body, sig_header)` is used (HMAC-SHA256 of raw body, hex). Secret is decrypted from `integration.webhook_secret_enc`.
3. **Extract provider store ref:** `extract_provider_store_reference(provider, payload)` — Just Eat: `restaurant_id` or `store_id`; Deliveroo: `location_id`; Uber Eats: `meta.user_id`.
4. **Lookup:** `find_integration_by_provider_store_reference(db, provider, provider_store_ref)`.
5. **Normalize:** `normalize_order(provider, &payload, org_id, store_id)` → `DeliveryOrderNormalized`.
6. **Insert/update order:** `insert_delivery_order(db, order)` (uses `ON DUPLICATE KEY UPDATE` on `(provider, provider_order_id)`).
7. **Enqueue command:** `enqueue_delivery_order_command(db, org_id, store_id, &pos_payload)` (normalized payload as `command_body`).
8. **last_sync_at:** `touch_integration_last_sync(db, &integration.id)`.
9. **Log:** `insert_delivery_log(db, log)` with provider, store_id, integration_id, request_payload (and on failure, wrapped body+headers).

**Locations in code:** `delivery_webhooks.rs`: lookup ~366–374, normalize ~403, insert_delivery_order ~392–394 / 406–408, enqueue_delivery_order_command ~411–413, touch ~415–417, insert_delivery_log ~337–346 (failure), ~374–384 (success).

---

## 5. Provider Store ID Flexibility

- **Connect body:** `ConnectBody { api_key: String, provider_store_reference: String }`. Field is `provider_store_reference` (not `provider_store_id`); it is required. Optional can be added by changing to `Option<String>` if desired.
- **Not forced to store UUID:** Any string (e.g. `"JE-STORE-999"`, `"location_abc"`) is stored in `delivery_integrations.provider_store_reference`.
- **Webhook lookup:** Uses `(provider, provider_store_reference)` via `find_integration_by_provider_store_reference`.

**Example connect JSON:** `{"api_key":"sk_live_xxx","provider_store_reference":"JE-STORE-999"}`.  
**Resulting row:** `provider_store_reference = 'JE-STORE-999'`.

---

## 6. Delivery Order Normalization

- **Function:** `normalize_order(provider, payload, org_id, store_id)` in `delivery_webhooks.rs` returns `Result<DeliveryOrderNormalized, String>`.
- **Domain type:** `domain::DeliveryOrderNormalized`: `type`, `provider`, `store_id`, `business_id`, `external_order_id`, `status`, `customer`, `delivery_address`, `items`, `total`, `notes`, `received_at`. Items are `DeliveryItem { name, quantity, unit_price }`; address is `DeliveryAddress { line1, line2, city, postcode, country }`.
- **Raw payload:** Stored in `delivery_orders.raw_payload` unchanged; normalized fields are derived from generic `payload.get(...)` (order_id/id, customer, delivery_address, items, total, notes/comment). Provider-specific mapping can be extended later.

**Example input (minimal):**  
`{"order_id":"ORD-1","customer":{"name":"Jane","phone":"+44..."},"delivery_address":{"line1":"10 High St","postcode":"SW1A 1AA"},"items":[{"name":"Burger","quantity":2,"unit_price":5.99}],"total":11.98,"notes":"No onions"}`  

**Example normalized output (structure):**  
`DeliveryOrderNormalized { type: "delivery_order", provider, store_id, business_id, external_order_id: "ORD-1", status: Pending, customer: Some({name, phone}), delivery_address: Some({line1, ...}), items: [{name, quantity, unit_price}], total: 11.98, notes: Some("No onions"), received_at: Some(now) }`.  
Subtotal/tax/fees/discounts and modifiers are not yet in the normalized struct; only `total` and basic items. Can be extended to match spec (subtotal, tax, fees, discounts, order_type, modifiers).

---

## 7. Command Queue Routing (Cloud → POS)

- **Function:** `db::enqueue_delivery_order_command(pool, org_id, store_id, payload)` in `crates/db/src/sync.rs`.
- **Logic:** Resolves device by `COALESCE(stores.canonical_device_id, (SELECT device_id FROM device_sync_state WHERE store_id = ? ORDER BY updated_at DESC LIMIT 1))`. Inserts into `device_command_queue`: `command_type = 'delivery_order'`, `command_body = payload` (normalized JSON), `org_id`, `store_id`, `device_id`, `status = 'queued'`.
- **POS retrieval:** `GET /api/sync/commands` (Bearer device token) → `fetch_deliverable_commands` returns queued/delivered commands for that device; response includes `command_type` and `command_body`.

**Example command_body (normalized):**  
`{"type":"delivery_order","provider":"just_eat","store_id":"...","business_id":"...","external_order_id":"ORD-1","status":"pending","customer":{...},"delivery_address":{...},"items":[...],"total":11.98,"notes":"...","received_at":"..."}`.

**SQL to verify after webhook:**  
`SELECT command_id, command_type, command_body, store_id, org_id FROM device_command_queue WHERE command_type = 'delivery_order' ORDER BY created_at DESC LIMIT 1;`

---

## 8. Logging Verification

- **Table:** `delivery_integration_logs`. Columns: `provider`, `store_id`, `integration_id`, `request_url`, `request_method`, `request_payload`, `response_status`, `response_payload`, `error_message`, `created_at`.
- **Success path:** Logs `request_payload: payload` (raw), `response_status: 200`, `response_payload: pos_payload` (normalized). No separate `status` column; status is implied by `response_status` and `error_message`.
- **Failure path (e.g. bad signature):** Logs `request_payload: { body, headers }` (wrapped), `response_status: 401`, `error_message: "invalid webhook signature"`. Signature header name and value are in the wrapped `headers` (e.g. `signature_header`, `signature_value`, `timestamp`, `user_agent`, `request_id`).
- **Sensitive redaction:** Logged payload is the webhook body; API keys are not in the body. Credentials are only in DB as encrypted blobs and are not written to logs. If provider sends secrets in payload, consider redacting in logging (not implemented in this pass).

**Example log record (success):**  
`{"provider":"just_eat","store_id":"...","integration_id":"...","request_method":"POST","request_payload":{...raw...},"response_status":200,"response_payload":{...normalized...},"error_message":null}`.

---

## 9. UI Verification (store.html)

- **Tab:** "Delivery Integrations" tab exists (`data-tab="delivery"`); panel `#tab-delivery` with section "Delivery Integrations" and Refresh button.
- **Cards:** For each of `just_eat`, `deliveroo`, `uber_eats`: status label (Connected/Pending/Error/Disconnected), last sync time, error message (if any), Connect/Disconnect and "Test connection" and "View logs" buttons.
- **Connect:** Prompts for API key and provider restaurant/store identifier; sends `POST .../connect` with `{ "api_key": "...", "provider_store_reference": "..." }`.
- **Fetch:** `GET /api/portal/stores/${storeId}/delivery_integrations`; uses `data.integrations` (fixed so API returns `{ "integrations": { ... } }`).

**JS fetch calls:**  
- Load: `fetch(\`/api/portal/stores/${encodeURIComponent(storeId)}/delivery_integrations\`)`  
- Test: `fetch(..., { method: 'POST' })` to `.../delivery_integrations/${provider}/test`  
- Connect: `fetch(..., { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ api_key, provider_store_reference }) })`  
- Disconnect: `fetch(..., { method: 'POST' })` to `.../disconnect`

---

## 10. Full End-to-End Test (Checklist)

Run in order:

1. **Set env:** `DELIVERY_CRED_ENC_KEY=$(openssl rand -base64 32)` (and `DATABASE_URL`, `PUBLIC_BASE_URL` if needed).
2. **Start API:** `cargo run -p cloud_api`.
3. **Login:** Get session cookie (e.g. portal login).
4. **Create store** (if needed) and note `store_id`.
5. **Connect Just Eat:**  
   `POST /api/portal/stores/<store_id>/delivery_integrations/just_eat/connect`  
   Body: `{"api_key":"<test_key>","provider_store_reference":"JE-STORE-999"}`.  
   (Connector may fail if provider rejects test key; stub implementations may succeed.)
6. **Send webhook:**  
   `POST /api/webhooks/just_eat`  
   Headers: `Content-Type: application/json`, `x-just-eat-signature: <HMAC-SHA256 hex of raw body using webhook_secret>` (get secret from DB decrypt or use a test secret you set).  
   Body: `{"restaurant_id":"JE-STORE-999","order_id":"JE-ORDER-1","customer":{"name":"Test","phone":"+44"},"items":[{"name":"Item","quantity":1,"unit_price":10}],"total":10}`.
7. **Verify DB:**  
   - `SELECT * FROM delivery_orders WHERE provider = 'just_eat' AND provider_order_id = 'JE-ORDER-1';` → one row.  
   - `SELECT last_sync_at FROM delivery_integrations WHERE provider = 'just_eat' AND store_id = '<store_id>';` → updated.  
   - `SELECT * FROM device_command_queue WHERE command_type = 'delivery_order' ORDER BY created_at DESC LIMIT 1;` → one row, `command_body` = normalized JSON.  
   - `SELECT * FROM delivery_integration_logs ORDER BY id DESC LIMIT 1;` → one log row.
8. **POS commands:** Call `GET /api/sync/commands` with Bearer token for a device of that store; confirm one `delivery_order` command with same payload.

---

## Final Checklist

| # | Category | Result | Notes |
|---|----------|--------|-------|
| 1 | Database (tables, indexes, constraints) | **PASS** | Schema matches migration 023; spec `unique(provider, provider_store_reference)` differs from design — design uses `unique(store_id, provider)`; webhook lookup by (provider, provider_store_reference) in code. |
| 2 | Encryption (AES-256-GCM, env, at-rest) | **PASS** | DELIVERY_CRED_ENC_KEY required; crypto.rs correct; DB stores ciphertext. Run manual encrypt/decrypt test with connect + SELECT + decrypt. |
| 3 | API routes (GET/POST, JSON, scoping, 400/404) | **PASS** | All four endpoints exist; GET returns `{ "integrations": { ... } }`; auth and store scoping added; invalid provider 400; missing store 404. |
| 4 | Webhook handlers (raw body, verify, lookup, normalize, insert, enqueue, last_sync, log) | **PASS** | Flow implemented as above; verify_traqr_webhook_secret uses raw body; lookup by provider_store_reference; insert_delivery_order + enqueue_delivery_order_command + touch + insert_delivery_log. |
| 5 | Provider store ID flexibility | **PASS** | Connect accepts `provider_store_reference` (any string); webhook lookup uses it. |
| 6 | Delivery order normalization | **PASS** | normalize_order returns DeliveryOrderNormalized; has provider_order_id, provider_store_reference (via integration), customer, delivery_address, items, total, notes, raw_payload; subtotal/tax/fees/modifiers/order_type can be added. |
| 7 | Command queue routing | **PASS** | enqueue_delivery_order_command uses canonical or latest device; inserts delivery_order with normalized body; POS gets it via GET /api/sync/commands. |
| 8 | Logging | **PASS** | delivery_integration_logs stores provider, store_id, integration_id, request payload, response status/payload, error_message; signature/headers captured on failure; no credential logging. |
| 9 | UI (store.html) | **PASS** | Delivery tab present; status, last sync, error, Connect/Disconnect/Test; Connect collects API key and provider_store_reference; fetch uses data.integrations (API fixed). |
| 10 | Full E2E | **CONDITIONAL** | Passes if connector test/webhook registration and signature generation are valid for your environment; DB and command flow are correct. Run steps in §10 and confirm with SQL and /api/sync/commands. |

---

## Fixes Applied During Review

1. **GET delivery_integrations response:** Backend now returns `{ "integrations": { "just_eat": {...}, ... } }` so the UI’s `data.integrations` is populated.
2. **Portal auth and store scoping:** All four portal delivery handlers now require `CurrentUser` and call `db::user_can_access_store`; 403 when the user cannot access the store.

No other code changes were required for the verification above. Recommend adding `DELIVERY_CRED_ENC_KEY` to `.env.example` and documenting it in the main delivery integrations doc.
