# Prompt: Sync your POS with Traqr Cloud menu and multi-store behaviour

Give this to the team (or AI) building or maintaining the **other application** (POS / till) so it stays in sync with the cloud.

---

## What the cloud now supports

1. **New store gets the menu from the cloud**  
   When a customer has multiple stores and opens a new one, they enter the same activation key (or a key for the new store). The new device should **pull the menu from the cloud** so it doesn’t start with an empty menu.

2. **Copy menu from another store (same org)**  
   The new device can ask the cloud for another store’s menu (same organization) and apply it locally, so the new location matches an existing one.

3. **Menu edits from the cloud**  
   Staff can tweak the menu in the Traqr Cloud portal (e.g. change names, prices, or availability). Those changes must **reach every device** in that store.

---

## What the POS must do

### 1. After activation: load menu from cloud

- **When:** Right after **POST /api/device/activate** succeeds and you have a `device_token` and `store_id`.
- **Call:** **GET /api/sync/menu** with header `Authorization: Bearer <device_token>`.
- **If the store already has a menu:** The response has `categories` and `items`. Apply this to your local menu (create/update categories and items using `local_category_id` and `local_item_id` as keys). Then continue normal sync (e.g. POST /api/sync/events for your own menu events if you want to stay in sync).
- **If this is a new store and you want to copy from another store in the same org:** Call **GET /api/sync/menu?copy_from_store_id=<uuid_of_existing_store>** with the same Bearer token. Use the returned `categories` and `items` to build your local menu, then sync menu events to the cloud so the cloud has the new store’s menu too.

### 2. Poll for commands and handle `apply_menu`

- **When:** You already poll **GET /api/sync/commands** for `void_order` / `refund_order`. Keep doing that.
- **New command type:** **`apply_menu`**
  - **When you see it:** The cloud is pushing an updated menu (e.g. after someone edited it in the portal).
  - **Payload:** `command_body` has the same shape as **GET /api/sync/menu**:
    - `command_body.categories`: array of `{ local_category_id, local_menu_id, name, position, image_path }`
    - `command_body.items`: array of `{ local_item_id, local_store_id, local_category_id, name, description, price_pence, active, image_path, customer_editable }`
  - **What to do:** Apply this menu to your local DB (replace or upsert categories and items keyed by `local_category_id` and `local_item_id`). Then call **POST /api/sync/commands/ack** with `command_id` and `status: "acked"` (or `"failed"` and a message if something went wrong).

### 3. Item images: upload first, then sync path

- When the user **adds or changes an image** for a menu item on the POS, the image must be stored in the cloud so the portal and other devices can display it.
- **Step 1:** Upload the image: **POST /api/sync/upload-item-image** with `Authorization: Bearer <device_token>` and multipart body (field name `file` or `image`). Response: `{ "url": "/uploads/menu/xxx.jpg", "path": "menu/xxx.jpg" }`.
- **Step 2:** Send the path in sync events:
  - **New item:** Include `image_path: response.path` in the **menu_item_created** event body.
  - **Existing item:** Send a **menu_item_image** event with `{ "item_id": "<local_item_id>", "image_path": "<response.path>" }`.
- If you do not upload the image and only send a local path (e.g. `file:///...`), the cloud will store it but the image will not be available in the portal or on other devices.

### 4. API reference

- Base URL: **`https://cloud.traqr.co.uk/api`** (set `CLOUD_API_URL` to this in the POS).
- **GET /api/sync/menu**
  - Headers: `Authorization: Bearer <device_token>`
  - Query (optional): `copy_from_store_id=<store_uuid>` (same org only).
  - Response: `{ "categories": [ ... ], "items": [ ... ] }` (see API_CONTRACT.md for field list).
- **POST /api/sync/upload-item-image**
  - Headers: `Authorization: Bearer <device_token>`
  - Body: multipart form with `file` or `image` (max 5MB; jpg/png/gif/webp).
  - Response: `{ "url": "/uploads/menu/...", "path": "menu/..." }` — use `path` as `image_path` in menu_item_created or menu_item_image events.
- **GET /api/sync/commands**  
  Now includes commands with `command_type === "apply_menu"` and `command_body` as above.
- **POST /api/sync/commands/ack**  
  Same as today; use it to ack `apply_menu` after applying the menu locally.

---

## Short “prompt” you can paste

You can paste this into your other application’s spec or into an AI assistant:

```
Our POS must sync with Traqr Cloud for menu and multi-store:

1) After device activation (we have device_token and store_id), call GET {CLOUD_API_URL}/sync/menu with Authorization: Bearer <device_token>. If the store has no menu yet but we want to copy from another store in the same org, call GET .../sync/menu?copy_from_store_id=<existing_store_uuid>. Apply the returned categories and items to our local menu (use local_category_id and local_item_id as keys).

2) For item images: when the user adds or changes an item image, POST the file to .../sync/upload-item-image (multipart, field "file" or "image"); use the response "path" as image_path in menu_item_created or in a menu_item_image event so the cloud stores and serves the image.

3) When we poll GET .../sync/commands, handle command_type "apply_menu": command_body has "categories" and "items" arrays (same shape as GET /sync/menu). Apply that menu to our local DB, then POST .../sync/commands/ack with that command_id and status "acked".

4) Full field list and errors are in the Traqr Cloud API contract (Sync: menu, Sync: upload item image, Sync: events, Sync: commands, apply_menu).
```

---

## Summary

| Cloud behaviour | POS action |
|-----------------|------------|
| New device activates in a new store | GET /api/sync/menu (or ?copy_from_store_id=other) and apply menu locally |
| User adds/changes item image on POS | POST /api/sync/upload-item-image, then send menu_item_created (with image_path) or menu_item_image event |
| Staff edits menu in portal | Cloud sends apply_menu command; POS applies command_body and acks |
