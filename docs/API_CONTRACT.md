# Traqr Cloud API Contract (Plan Earle)

Shared types live in the `domain` crate. This document describes the device and sync endpoints.

---

## Base URL

- **Base URL (production):** **`https://cloud.traqr.co.uk`** — web app at `/`, API at `/api`.
- **Web app:** `/` — Product, Pricing, Devices, Contact, Login (static HTML + Tailwind).
- **API (for POS):** All API routes are under **`/api`** on the same host.
- Development: `http://localhost:8080` (web at `/`, API at `http://localhost:8080/api`).

The POS is configured via **CLOUD_API_URL** = **`https://cloud.traqr.co.uk/api`**. All device and sync endpoints are relative to that base.

---

## Admin: activation keys

**POST /api/admin/activation-keys**

Create an org/store if needed and issue an activation key. The raw key is returned **once**; the operator pastes it into the POS (Settings → Cloud or Setup → Traqr Cloud). Optional: set `ADMIN_API_KEY` and send `X-Admin-Key: <value>` to protect this endpoint.

**Headers (optional):**

| Header | Description |
|--------|-------------|
| `X-Admin-Key` | Must match `ADMIN_API_KEY` env if that is set |

**Request (JSON):** Either existing org/store or create by name:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `org_id` | UUID | No* | Existing org (with `store_id`) |
| `store_id` | UUID | No* | Existing store |
| `org_name` | string | No* | Create org with this name |
| `org_slug` | string | No* | Org slug (unique) |
| `store_name` | string | No* | Create store with this name |
| `scope_type` | string | Yes | `store`, `franchise`, or `org` |
| `scope_id` | UUID | No | For `store`, use store_id |
| `max_uses` | number | No | Max activations (null = single use) |
| `expires_at` | string | No | RFC3339 or null |

\* Provide either (`org_id` + `store_id`) or (`org_name` + `org_slug` + `store_name`).

**Response (200):**

| Field | Type | Description |
|-------|------|-------------|
| `activation_key` | string | **Raw key — show once.** Paste into POS. |
| `key_id` | UUID | Key record id |
| `org_id` | UUID | Organization id |
| `store_id` | UUID | Store id |
| `scope_type` | string | As requested |
| `scope_id` | UUID \| null | As requested |
| `max_uses` | number \| null | As requested |
| `expires_at` | string \| null | As requested |

**Errors:** 400 invalid scope or missing params; 401 invalid/missing X-Admin-Key when ADMIN_API_KEY set; 500 server error.

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
| `device_name` | string | No | Display name for this device (e.g. "Till 1", "Kitchen screen"). From POS Setup. |
| `is_primary` | boolean | No | `true` if this device is the primary (authority) for the store; `false` for secondaries. |

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

## Sync: menu (device ← cloud)

**GET /api/sync/menu**

Returns the current menu (categories and items) for the device’s store. Use this after activation so a new device can load the same menu as other stores in the org, or to pull another store’s menu when opening a new location.

**Headers:**

- `Authorization: Bearer <device_token>`

**Query:**

| Param | Type | Description |
|-------|------|-------------|
| `copy_from_store_id` | UUID | Optional. Same-org store id to copy menu from (e.g. new store gets menu from an existing store). If omitted, returns the menu for the device’s own store. |

**Response (200):**

| Field | Type | Description |
|-------|------|-------------|
| `categories` | array | Menu categories (see below) |
| `items` | array | Menu items (see below) |

Each category:

| Field | Type | Description |
|-------|------|-------------|
| `local_category_id` | string | POS local category id |
| `local_menu_id` | string | POS local menu id (e.g. `"default"`) |
| `name` | string | Display name |
| `position` | number | Sort order |
| `image_path` | string \| null | Optional image path |

Each item:

| Field | Type | Description |
|-------|------|-------------|
| `local_item_id` | string | POS local item id |
| `local_store_id` | string \| null | Optional store scope |
| `local_category_id` | string \| null | Category id |
| `name` | string | Display name |
| `description` | string \| null | Optional description |
| `price_pence` | number \| null | Price in pence |
| `active` | boolean | Visible/sellable |
| `image_path` | string \| null | Optional image path |
| `customer_editable` | boolean | Whether customer can edit (e.g. notes) |

**Behaviour:**

- With no query: returns the menu for the device’s store (from the cloud read model). If that store has no menu yet (no device has synced), returns empty `categories` and `items`.
- With `copy_from_store_id`: must be a store in the same organization. Returns that store’s menu so the POS can apply it locally (e.g. new store copies menu from existing store), then sync menu events to the cloud as usual.

**Errors:** 401 missing/invalid device token; 403 Cloud Sync not enabled or store not in your org; 500 server error.

---

## Sync: upload item image (device → cloud)

**POST /api/sync/upload-item-image**

Upload an image for a menu item. Use the returned `path` in **menu_item_created** or **menu_item_image** sync events so the cloud (and other devices) can serve the image. Requires device auth and Cloud Sync entitlement.

**Headers:**

- `Authorization: Bearer <device_token>`

**Request:** `multipart/form-data` with a file part. Preferred field names: `file` or `image`; the cloud also accepts any part that has a filename. Max size 5MB. Allowed types: jpg, jpeg, png, gif, webp.

**Response (200):**

| Field | Type | Description |
|-------|------|-------------|
| `url` | string | Full URL path to serve the image, e.g. `"/uploads/menu/uuid.jpg"` |
| `path` | string | Relative path to store in events, e.g. `"menu/uuid.jpg"` — send this as `image_path` in **menu_item_created** or **menu_item_image** |

**Flow:** When the user adds or changes an item image on the POS: (1) POST the image to this endpoint; (2) send a **menu_item_created** event (new item) with `event_body.image_path` set to the response `path`, or send a **menu_item_image** event (existing item) with `event_body.item_id` and `event_body.image_path` set to the response `path`.

**Errors:** 401 missing/invalid device token; 403 Cloud Sync not enabled; 400 missing/invalid multipart; 413 file too large (max 5MB); 500 server error.

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
| `event_type` | string | Yes | e.g. `order_created`, `transaction_completed`, `device_updated` |
| `occurred_at` | string | Yes | RFC3339 timestamp (device time) |
| `event_body` | object | Yes | Event payload (JSON) |

**Device events:** For `event_type` **`device_updated`**, the cloud updates the device’s display name and primary flag. Body: `{ "device_name": "Till 1", "is_primary": true }`. Sent after activation and when the user changes device name or primary in POS Setup.

**Menu item images (sync from POS):** For images to appear in the cloud and portal, the POS must (1) upload the file to **POST /api/sync/upload-item-image**, then (2) send the returned `path` in a sync event:

- **New item with image:** `event_type`: `"menu_item_created"`. `event_body` must include `"image_path": "<path from upload response>"` (e.g. `"menu/uuid.jpg"`). Other fields: `item_id`, `store_id` (optional), `category_id` (optional), `name`, `description` (optional), `price_pence` or `price` (optional), `active` (optional, default true), `customer_editable` (optional).
- **Existing item – set or change image:** `event_type`: `"menu_item_image"`. `event_body`: `{ "item_id": "<local item id>", "image_path": "<path from upload response>" }`. The cloud updates that item’s image; use the same `item_id` the POS uses for that menu item (same as in `menu_item_created`).

If the POS sends a local path (e.g. `file:///...`) without uploading the file, the cloud stores the string but cannot serve the image.

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

**Command body shape (void_order / refund_order):** The POS uses its own SQLite order ids, not cloud UUIDs. When the portal enqueues `void_order` or `refund_order`, the command body **must** include the POS local order id so the device can find the order. Use either key:

- `local_order_id` (string) — preferred, e.g. `{ "local_order_id": "abc-123-uuid-from-pos" }`
- `order_id` (string) — accepted alias

Example: `{ "local_order_id": "abc-123-uuid-from-pos" }` for void and refund. The portal gets this value from the orders read model (`orders.local_order_id`, populated from `event_body.order_id` when events are received).

**Command type `apply_menu`:** When the portal (or cloud) edits the menu, the cloud enqueues an `apply_menu` command for each device in the store. The POS should apply the payload to its local menu (replace or merge categories and items using `local_category_id` and `local_item_id`). Command body shape:

- `categories`: array of `{ "local_category_id", "local_menu_id", "name", "position", "image_path" }`
- `items`: array of `{ "local_item_id", "local_store_id", "local_category_id", "name", "description", "price_pence", "active", "image_path", "customer_editable" }`

Same shape as **GET /api/sync/menu** response. After applying, the POS should ack the command with `status: "acked"`.

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

## Portal: enqueueing commands

The portal (or any backend) enqueues commands for the POS by inserting into `device_command_queue`:

- `command_id` — new UUID
- `org_id`, `store_id`, `device_id` — from the order/device (use the device that owns the order)
- `command_type` — e.g. `void_order`, `refund_order`
- `command_body` — JSON; for `void_order` and `refund_order` **must** include the POS local order id: `{ "local_order_id": "<orders.local_order_id>" }` (or `"order_id"`). Get `orders.local_order_id` from the orders read model (populated from `event_body.order_id` when events are received).
- `status` — `queued`
- `sensitive` — 0 or 1 (e.g. 1 if two-person approval required)

The POS polls GET /api/sync/commands and will receive the command; it looks up the order by `command_body.local_order_id` (or `order_id`) in its local SQLite and executes void/refund there.

For **apply_menu**, the cloud inserts a row with `command_type = 'apply_menu'` and `command_body = { "categories": [...], "items": [...] }` (same shape as GET /api/sync/menu). The POS applies that menu and acks the command.

---

## Rust types (domain crate)

- `ActivateDeviceRequest`, `ActivateDeviceResponse`
- `SyncEventsRequest`, `SyncEventsResponse`, `DeviceEventIn`
- `SyncCommandsResponse`, `DeviceCommandOut`, `CommandAckRequest`

These can be shared with the POS client (e.g. via a shared crate or generated from OpenAPI).
