# Traqr Cloud Platform — Plan Earle Implementation

This document maps the **Plan Earle** specification to the Traqr Cloud codebase, records what exists vs gaps, and tracks rollout phases.

---

## 1. Goal & Non-Goals (v1)

| Goal | Status |
|------|--------|
| Multi-tenant cloud under Traqr subdomain (cloud.traqr.co.uk / api.traqr.co.uk) | **Planned** — deployment/hosting TBD |
| Integrate with existing POS (Rust/Axum/SQLite) offline-first | **Planned** — API contract + sync protocol defined |
| Marketing + account pages (Tailwind) | **Planned** — UI crate scaffold only |
| Franchise/orgs, stores, devices, paid cloud sync add-on | **Partial** — tenancy + devices in DB; entitlements/billing not yet |
| Incremental sync: Device→Cloud events (outbox), Cloud→Device commands (inbox) | **Partial** — sync tables + stub endpoints; events not persisted yet |
| Device authoritative; two-person approval for sensitive commands | **Partial** — approvals table exists; workflow not wired |

| Non-Goal (v1) | Notes |
|---------------|--------|
| Running POS UI from cloud | Out of scope |
| Real-time websockets | HTTP polling only |
| Complex multi-writer conflict resolution | Device is source of truth |
| Store override of pricing/item definitions | Availability/stock only |

---

## 2. Product Rules (Locked)

- **Menus:** Head office publishes; stores override availability/visibility only.
- **Orders/transactions:** Created on-device; cloud replica for reporting; changes via commands.
- **Two-person approval** (cloud portal) for refund/void/order-changing commands.
- **Sync:** Append-only events up; approved commands down; idempotent by `(device_id, event_id)` and `command_id`.

---

## 3. POS Data Model to Mirror (Reference)

Plan Earle references these POS migrations (path `/var/traqr_pos/migrations/` — adjust if your POS lives elsewhere):

| Area | POS Tables | Cloud mirror / usage |
|------|------------|----------------------|
| Menus | `menus`, `menu_categories`, `menu_items` (004_menus.sql) | `menu_drafts`, `menu_publishes`, `store_menu_overrides` |
| Stock/availability | `dish_yields`, `yield_adjustments` (005_dish_yields.sql) | Store overrides only |
| Orders | `orders`, `order_items` (006_orders.sql) | `orders`, `order_items` (read model) |
| Payments | `transactions` (007_transactions.sql) | `transactions` (read model) |
| Receipts | `receipts` (008_receipts.sql) | `receipts` (read model) |
| Users/roles (POS staff) | `users`, `roles`, `user_roles` (003_users_roles.sql) | Not mirrored in cloud; cloud has `cloud_users` / `cloud_roles` |
| Devices | `devices` (002_devices.sql) | `devices` (cloud view), `device_activation_keys`, `device_tokens` |

---

## 4. Cloud Architecture vs Codebase

| Component | Spec | Current state |
|-----------|------|---------------|
| **API** | Rust, Axum, public + portal + sync endpoints | ✅ `cloud_api` (Axum), `/health`, `/device/activate`, `/sync/events`, `/sync/commands`, `/sync/commands/ack` |
| **PostgreSQL** | Canonical data + audit | ✅ Migrations 001–006; missing: entitlements, orders/transactions/receipts/order_events, menu_drafts/publishes/overrides, command status/expiry |
| **Tenant model** | org → franchise → stores, memberships | ✅ `organizations`, `franchises`, `stores`, `org_memberships`, `store_memberships` |
| **Auth** | cloud_users, cloud_roles, session/JWT | ✅ Tables; no session/JWT or portal auth yet |
| **Devices** | devices, activation keys, tokens | ✅ Tables; activate endpoint is stub (no DB validation or token creation) |
| **Sync protocol** | device_event_log, device_sync_state, device_command_queue, approvals | ✅ Tables; events not persisted; commands not fetched from DB |
| **Object storage** | S3-compatible (menu images) | ❌ `storage` crate empty |
| **Billing** | Subscriptions + metering (cloud sync add-on) | ❌ No plans/entitlements/purchases tables yet |

---

## 5. Sync Protocol (Explicit)

| Endpoint | Spec | Current |
|----------|------|---------|
| **POST /sync/events** | Idempotent insert by `(device_id, event_id)`; update read models; return ack watermark | Stub: returns computed ack_seq; no DB insert |
| **GET /sync/commands?since=...** | Deliverable (approved/queued) commands for device | Stub: returns empty list |
| **POST /sync/commands/ack** | Mark command acked/failed; audit | Stub: 200 OK only |

**Device auth:** Plan says use `device_token` for sync. Current sync routes do not validate token.

---

## 6. Two-Person Approval Workflow

- User A initiates sensitive action → `pending_command` + `approval_request`.
- User B (distinct, role policy) approves → command becomes `queued` / deliverable.
- Optional: support role can approve.
- Current: `approvals` table exists; `device_command_queue` needs `status` enum (queued/delivered/acked/failed/expired) and link to approval policy.

---

## 7. UI/Pages (Tailwind)

| Page type | Spec | Current |
|-----------|------|--------|
| Marketing | Product, pricing, devices, contact | ❌ Not started |
| Portal | Org/franchise/stores, devices, entitlements, menu publish, store overrides, orders, command center | ❌ `ui` crate scaffold only |

---

## 8. Security Requirements

| Requirement | Status |
|-------------|--------|
| Strict tenant scoping on every query | ❌ Not enforced in handlers yet |
| Device tokens rotateable; revoke on lost device | Table supports revoke; no rotate/revoke API yet |
| Command execution idempotent (command_id) | ✅ Design; not implemented in device flow |
| Audit: approvals, commands, refunds | Tables present; audit logging not wired |

---

## 9. Rollout Phases — Mapping to Tasks

### Phase 1: Foundations ✅ (mostly done)

- [x] Postgres schema org/franchise/store/users/devices (tenancy, auth, devices)
- [x] Sync tables: device_event_log, device_sync_state, device_command_queue, approvals
- [x] **Entitlements:** plans, org_entitlements, device_entitlements (migration 009)
- [x] Activation key + device token issuance (design + tables + POST /device/activate implemented)
- [ ] **Portal scaffolding (Tailwind):** minimal layout + auth placeholder

### Phase 2: Sync v1 (events up)

- [x] **POST /sync/events:** idempotent insert into `device_event_log`, update `device_sync_state.last_ack_seq`; device auth via Bearer token
- [ ] **Read model:** from events, maintain `orders`, `order_items`, `transactions`, `receipts`, `order_events` (tables in 008; population TBD)
- [x] Device auth: validate `device_token` on sync routes (Bearer token, hash lookup in device_tokens)

### Phase 3: Menus publish + store overrides

- [ ] Migrations: menu_drafts, menu_publishes, store_menu_overrides
- [ ] Command type: `apply_menu_publish`
- [ ] Store override events (availability only) in event log and overrides table

### Phase 4: Two-person approvals + commands down

- [x] Command status: queued / delivered / acked / failed / expired (migration 007)
- [ ] Approval policies + audit views
- [x] GET /sync/commands from DB (deliverable only); POST /sync/commands/ack updates status; device-scoped
- [ ] Command types: void_order, refund_order, set_order_status, apply_menu_publish (queue populated by portal)

### Phase 5: Billing integration

- [ ] Subscription tiers + add-on (cloud sync) enforcement
- [ ] Entitlement checks at activation and during sync

---

## 10. Handover Checklist (Earle → Next)

- [ ] **API contract:** OpenAPI or shared Rust structs (domain crate) — partially in place
- [ ] **Postgres migrations:** All Phase 1–4 tables versioned and runnable
- [ ] **Seed/admin tooling:** Create org/franchise/store, issue activation keys
- [ ] **Command + approvals UX:** Document flows for portal (initiate → approve → deliver → ack)
- [ ] **Test plan:** Idempotency (events, acks), tenant isolation (no cross-tenant data leak)

---

## 11. File / Crate Reference

| Path | Purpose |
|------|--------|
| `migrations/001_extensions.sql` | citext, pgcrypto |
| `migrations/002_tenancy.sql` | organizations, franchises, stores |
| `migrations/003_auth.sql` | cloud_users, cloud_roles, org_memberships, store_memberships |
| `migrations/004_devices.sql` | devices, device_activation_keys, device_tokens |
| `migrations/005_sync.sql` | device_event_log, device_sync_state, device_command_queue, approvals |
| `migrations/006_seed.sql` | cloud_roles seed |
| `crates/cloud_api` | Axum HTTP API |
| `crates/domain` | Shared request/response types |
| `crates/db` | PgPool + connect |
| `docs/ARCHITECTURE.md` | High-level architecture |
| `docs/DATABASE_SCHEMA.md` | Schema overview |
| `docs/SYNC_PROTOCOL.md` | Sync endpoints and behaviour |

This file is the **single Plan Earle implementation map**. Update it as phases are completed.
