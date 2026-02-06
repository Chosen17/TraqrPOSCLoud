## Traqr Cloud readiness audit (2026-02-06)

This is a **code-based** audit of what is implemented vs partial vs not implemented in the current repo (`/var/TraqrPOSCLoud`), focusing on the areas you called out: **sync completeness** and **delivery integrations**, plus a quick “launch-risk” pass.

### Executive summary (high-signal)

- **Critical bug found (fixed in code):** `/api/portal/orders/:order_id` could crash the server due to a SQLx decode mismatch (`DECIMAL` → `f64`) when reading `order_items.quantity`. This made the UI look like “sync is missing” when the data actually exists in the DB.
  - Fix: cast `quantity` to `DOUBLE` in the query.
  - File: `crates/cloud_api/src/routes/portal_orders.rs`

- **Critical security gaps found (fixed in code):**
  - `/api/portal/orders/:order_id` and `/api/portal/orders/:order_id/commands` were missing portal auth + store access checks (anyone with an order UUID could fetch/enqueue).
  - `/api/stores/:store_id/delivery_orders` was unauthenticated.
  - Fix: require `CurrentUser` + `db::user_can_access_store`.
  - Files: `crates/cloud_api/src/routes/portal_orders.rs`, `crates/cloud_api/src/routes/delivery_webhooks.rs`

### What’s “complete” (implemented end-to-end)

#### 1) Device activation (POS → Cloud bootstrap) — **Complete**

- **Endpoint:** `POST /api/device/activate`
- **What it does:** validates activation key, enforces org entitlement `cloud_sync`, resolves store, creates device, creates device token, initializes sync state, increments activation key uses.
- **Files:** `crates/cloud_api/src/routes/device_activate.rs`, `crates/db/src/device.rs` (and related db helpers)

#### 2) Sync (events up) — **Complete**

- **Endpoint:** `POST /api/sync/events`
- **What it does:** device Bearer token validation, entitlement gating, idempotent insert into `device_event_log`, projection into read models, update `device_sync_state.last_ack_seq`.
- **Files:** `crates/cloud_api/src/routes/sync_events.rs`, `crates/db/src/sync.rs`, `crates/db/src/read_model.rs`, `crates/db/src/orders.rs`

#### 3) Sync (commands down) — **Complete**

- **Endpoints:** `GET /api/sync/commands`, `POST /api/sync/commands/ack`
- **What it does:** device Bearer token validation, entitlement gating, fetch queued/delivered commands, mark delivered, accept ack/failed updates.
- **Files:** `crates/cloud_api/src/routes/sync_commands.rs`, `crates/db/src/sync.rs`

#### 4) Menu sync and image upload — **Complete**

- **Endpoints:** `GET /api/sync/menu`, `POST /api/sync/upload-item-image`, plus `GET /uploads/*`
- **What it does:** device Bearer token validation + entitlement gating, returns menu, supports image uploads to `UPLOAD_DIR`.
- **Files:** `crates/cloud_api/src/routes/sync_menu.rs`, `crates/cloud_api/src/main.rs`

#### 5) Portal auth (signup/login/sessions) — **Complete**

- **Endpoints:** `POST /api/auth/signup`, `POST /api/auth/login`, `POST /api/auth/logout`, `GET /api/auth/me`
- **What it does:** creates user/org/store, issues session cookie `traqr_session`, validates sessions for portal routes via `CurrentUser`.
- **Files:** `crates/cloud_api/src/routes/auth_login.rs`, `crates/cloud_api/src/session.rs`

#### 6) Orders read model + portal order view — **Complete (after fixes)**

- **Projection:** `order_created`, `order_updated`, `transaction_completed`, `receipt_created` into tables `orders`, `order_items`, `transactions`, `receipts`.
- **Portal API:** `GET /api/portal/orders/:order_id`, `POST /api/portal/orders/:order_id/commands` (void/refund -> `device_command_queue`)
- **Files:** `crates/db/src/orders.rs`, `migrations/008_orders_read_model.sql`, `crates/cloud_api/src/routes/portal_orders.rs`

### What’s “partial” (implemented but with real gaps)

#### 7) Delivery integrations (overall pipeline) — **Partial**

**What’s implemented end-to-end:**

- Store-scoped integrations table + encrypted credentials at rest.
- Connect/test/disconnect endpoints (portal-authenticated).
- Webhook ingestion for `just_eat`, `deliveroo`, `uber_eats`.
- Normalization into a common `DeliveryOrderNormalized`.
- Persist into `delivery_orders` + enqueue POS command `delivery_order` to canonical/latest device.
- Logging into `delivery_integration_logs`.

**Files:** `crates/cloud_api/src/routes/delivery_webhooks.rs`, `crates/cloud_api/src/crypto.rs`, `crates/cloud_api/src/delivery_connectors/*`, `migrations/023_delivery_integrations.sql`, `crates/db/src/delivery_integrations.rs`, `crates/db/src/sync.rs`

**Key gaps (meaning: not “launch-complete”):**

- **Deliveroo connector is stubbed:** `test_connection()` always OK (“stubbed OK”). Webhook verification is implemented, but there is no real credential validation.
  - File: `crates/cloud_api/src/delivery_connectors/deliveroo.rs`
- **Just Eat webhook verification is disabled** (`WebhookVerificationStrategy::None`), and connection testing is only “real” if `JUST_EAT_TEST_URL` is set.
  - File: `crates/cloud_api/src/delivery_connectors/just_eat.rs`
- **Webhook registration is a no-op** for all providers (`register_webhook()` returns Ok without calling provider APIs). This means setup depends on manual provider-portal configuration.
  - Files: `crates/cloud_api/src/delivery_connectors/{uber_eats,deliveroo,just_eat}.rs`
- **Normalization is generic for non-Uber and currently defaults status to `pending`** (no provider-specific status mapping except Uber’s “full order fetch” path).
  - File: `crates/cloud_api/src/routes/delivery_webhooks.rs`

#### 8) Two-person approval workflow for sensitive commands — **Not implemented**

- The portal can enqueue `void_order` / `refund_order` directly as `sensitive=1` in `device_command_queue`.
- There is **no approval request/approval enforcement** in the API surface we reviewed.
- Files involved today: `crates/cloud_api/src/routes/portal_orders.rs`, `crates/db/src/sync.rs`
- (Docs mention this requirement, but code does not implement it.)

### “Not implemented” (explicit)

- **Deliveroo real API auth/validation** in connector.
- **Just Eat webhook signature verification** (currently disabled).
- **Provider-managed webhook registration automation** (connectors don’t call provider APIs).
- **Two-person approvals** (workflow + policy + portal UX).

### Why your `/order` page looked “not fully synced”

Even when the DB has full data, `/order` depends on `GET /api/portal/orders/:id`. That endpoint was crashing while decoding `order_items.quantity`:

- DB type: `DECIMAL(18,4)`
- Rust decode attempted: `f64`
- Result: server panic → browser sees an empty/failed response → page looks like “sync didn’t bring items/receipts”.

This is now fixed in code by casting `quantity` to `DOUBLE` in SQL.

### Immediate launch-critical checklist (high priority)

- [ ] Deploy the patched build (order endpoint crash + auth fixes).
- [ ] Decide policy for **delivery providers** at launch:
  - Uber Eats: can be “launchable” sooner.
  - Deliveroo/Just Eat: currently not at parity (stubbed/verification gaps).
- [ ] Implement two-person approval workflow (or explicitly remove the requirement and document why).
- [ ] Run a full tenant-isolation sweep of portal endpoints (we fixed two concrete leaks).

