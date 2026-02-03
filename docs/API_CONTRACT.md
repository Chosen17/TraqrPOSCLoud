# Traqr Cloud API Contract (Plan Earle)

Shared types live in the `domain` crate. This document describes the device and sync endpoints.

---

## Base URL

- **Web app (cloud.traqr.co.uk):** `/` — Product, Pricing, Devices, Contact, Login (static HTML + Tailwind).
- **API (for POS):** All API routes are under **`/api`** on the same host, or a separate host (e.g. api.traqr.co.uk).
- Development: `http://localhost:8080` (web at `/`, API at `http://localhost:8080/api`)
- Production: `https://cloud.traqr.co.uk` (web), `https://cloud.traqr.co.uk/api` or `https://api.traqr.co.uk` (API)

---

## Device activation

**POST /api/device/activate**

Activates a device using an activation key. Creates a device record, issues a one-time device token, and initializes sync state.

**Request (JSON):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `local_device_id` | string | Yes | POS local device identifier (for reference) |
| `activation_key` | string | Yes | Key issued by cloud (stored hashed in `device_activation_keys`) |
| `store_hint` | UUID | No | When key scope is franchise/org, hints which store to assign |

**Response (200):**

| Field | Type | Description |
|-------|------|-------------|
| `device_id` | UUID | Cloud device id |
| `org_id` | UUID | Organization id |
| `store_id` | UUID | Store (location) id |
| `device_token` | string | **Returned once**; use as Bearer token for sync |
| `polling_interval_seconds` | number | Suggested interval for polling commands |

**Errors:** 401 invalid/expired/maxed key; 400 store resolution failed; 500 server error.

---

## Sync: events (device → cloud)

**POST /api/sync/events**

Append-only event upload. Idempotent by `(device_id, event_id)`. Requires device auth.

**Headers:**

- `Authorization: Bearer <device_token>`

**Request (JSON):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `last_ack_seq` | number \| null | No | Last sequence number cloud acknowledged |
| `events` | array | Yes | Events to upload |

Each event:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `event_id` | UUID | Yes | Idempotency key (unique per device) |
| `seq` | number \| null | No | Monotonic sequence (used for ack watermark) |
| `event_type` | string | Yes | e.g. `order_created`, `transaction_completed` |
| `occurred_at` | string | Yes | RFC3339 timestamp (device time) |
| `event_body` | object | Yes | Event payload (JSON) |

**Response (200):**

| Field | Type | Description |
|-------|------|-------------|
| `ack_seq` | number \| null | Watermark: cloud has persisted events up to this seq |

**Errors:** 401 missing/invalid device token; 400 invalid `occurred_at`; 500 server error.

---

## Sync: commands (cloud → device)

**GET /api/sync/commands**

Returns deliverable commands (status `queued` or `delivered`) for the authenticated device. After return, commands are marked `delivered`.

**Headers:**

- `Authorization: Bearer <device_token>`

**Query:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `limit` | number | 50 | Max commands to return (capped at 200) |

**Response (200):**

| Field | Type | Description |
|-------|------|-------------|
| `commands` | array | List of commands |

Each command:

| Field | Type | Description |
|-------|------|-------------|
| `command_id` | UUID | Idempotency key; use in ack |
| `command_type` | string | e.g. `void_order`, `refund_order`, `apply_menu_publish` |
| `sensitive` | boolean | True if two-person approval was required |
| `command_body` | object | Payload (JSON) |

**Errors:** 401 missing/invalid device token; 500 server error.

---

**POST /api/sync/commands/ack**

Ack or fail a command after execution on device. Idempotent by `command_id`; only the owning device can ack.

**Headers:**

- `Authorization: Bearer <device_token>`

**Request (JSON):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `command_id` | UUID | Yes | Command id from GET /sync/commands |
| `status` | string | Yes | `acked` or `failed` |
| `result` | object \| null | No | Optional result payload (e.g. local order id, error message) |

**Response:** 200 OK, or 404 if command not found / already acked or failed.

**Errors:** 401 missing/invalid device token; 400 status not `acked`/`failed`; 404 command not found or already terminal; 500 server error.

---

## Rust types (domain crate)

- `ActivateDeviceRequest`, `ActivateDeviceResponse`
- `SyncEventsRequest`, `SyncEventsResponse`, `DeviceEventIn`
- `SyncCommandsResponse`, `DeviceCommandOut`, `CommandAckRequest`

These can be shared with the POS client (e.g. via a shared crate or generated from OpenAPI).
