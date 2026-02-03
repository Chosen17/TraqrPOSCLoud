# Plan Earle — Handover Checklist

What Earle (or the current implementer) should hand over for the next phase.

---

## 1. API contract

- [x] **Rust structs** in `domain` crate: request/response types for activate, sync/events, sync/commands.
- [ ] **OpenAPI** (optional): add `openapi` / `utoipa` to generate spec from routes for external clients.

**Location:** `crates/domain/src/lib.rs`, `docs/API_CONTRACT.md`.

---

## 2. Postgres migrations

- [x] 001_extensions, 002_tenancy, 003_auth, 004_devices, 005_sync, 006_seed.
- [x] 007_command_status (command status check, expires_at, delivered_at, ack_result).
- [x] 008_orders_read_model (orders, order_items, transactions, receipts, order_events).
- [x] 009_entitlements (plans, org_entitlements, device_entitlements).

**Run order:** 001 → 009. Use `sqlx migrate run` or your migration runner with `DATABASE_URL`.

---

## 3. Seed / admin tooling

- [ ] **Create org/franchise/store:** script or admin API to insert into `organizations`, `franchises`, `stores`.
- [ ] **Issue activation keys:** generate a secret, store SHA-256 in `device_activation_keys` with scope (store/franchise/org), optional max_uses and expires_at. Expose the raw key once to the customer (e.g. print or copy).

**Suggested:** CLI or small admin service that uses the same `db` crate and migrations.

---

## 4. Command + approvals UX (documentation)

- [ ] **Flow:** User A (portal) initiates sensitive action → system creates row in `device_command_queue` with `sensitive=true` and status `queued`, and creates `approval_request` (or uses `approvals` when User B approves). User B (distinct, allowed role) approves → command remains `queued` (deliverable). Device GET /sync/commands receives it; device executes and POST /sync/commands/ack with status acked/failed.
- [ ] **Policy:** Document which roles can approve (e.g. head_office_ops, finance, support). Two distinct users required for sensitive commands.
- [ ] **Audit:** All approvals and command state changes should be visible (approvals table + command status/ack_result).

**Location:** Add to `docs/SYNC_PROTOCOL.md` or new `docs/COMMAND_APPROVALS.md`.

---

## 5. Test plan

- [ ] **Idempotency:** POST /sync/events with same (device_id, event_id) twice → 200 both times, single row in device_event_log. POST /sync/commands/ack for same command_id twice → second returns 404 or no-op.
- [ ] **Tenant isolation:** Device A (org1/store1) cannot see or ack commands for device B (org2/store2). Queries must always filter by org_id/store_id/device_id from authenticated identity.
- [ ] **Device auth:** Requests without Bearer token or with revoked/wrong token → 401.

**Suggested:** Integration tests (e.g. `tests/` in cloud_api or workspace) with a test DB and sqlx offline mode or testcontainers.

---

## 6. Implemented so far (Phase 1–2)

- **Device activation:** POST /device/activate validates activation key (by SHA-256 hash), resolves store, creates device + device_token (hashed) + device_sync_state, increments key use count.
- **Sync events:** POST /sync/events requires Bearer device token; idempotent insert into device_event_log; updates device_sync_state.last_ack_seq.
- **Sync commands:** GET /sync/commands requires Bearer device token; returns deliverable commands from device_command_queue; marks them delivered. POST /sync/commands/ack requires Bearer token; marks command acked/failed with device-scoping.
- **Migrations:** Command status/expiry, orders read model, entitlements tables added; read-model population from events is Phase 2 follow-up (not yet implemented).

---

## 7. Next owner actions

1. Run migrations 007–009 on target DB.
2. Add seed/admin tooling for orgs and activation keys.
3. Document approval workflow and add tests for idempotency and tenant isolation.
4. (Phase 2) Consume device_event_log to populate orders/order_items/transactions/receipts and order_events.
5. (Phase 3+) Menu drafts/publishes, store overrides, two-person approval enforcement in portal.
