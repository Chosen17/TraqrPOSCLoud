# Item images: syncing from the POS

Item images in the cloud (and in the portal) **sync from the POS** when the POS does both of these:

1. **Upload the image** to the cloud: `POST /api/sync/upload-item-image` (multipart, Bearer device token).  
   The cloud saves the file and returns `{ "path": "menu/xxx.jpg", "url": "/uploads/menu/xxx.jpg" }`.

2. **Send the path in a sync event** via `POST /api/sync/events`:
   - **New item:** include `"image_path": "<path>"` in the **menu_item_created** event body.
   - **Existing item:** send a **menu_item_image** event with `"item_id"` and `"image_path": "<path>"`.

The cloud then stores the path on the menu item and serves the image at `/uploads/<path>`. The portal and other devices (via `apply_menu`) get the updated menu with images.

---

## What to give the POS project

Use **docs/POS_ITEM_IMAGE_PROMPT.md** as the single prompt for the POS team (or paste it into the other repo). It describes:

- Upload endpoint and request format.
- When to send **menu_item_created** with `image_path` and when to send **menu_item_image**.
- Order of operations (upload first, then send event with `path`).
- What not to do (e.g. don’t send a local path without uploading).

API details (errors, fields): **docs/API_CONTRACT.md** (Sync: upload item image, Sync: events).

---

## Cloud behaviour (already implemented)

- **POST /api/sync/upload-item-image** – device auth; saves file under `UPLOAD_DIR/menu/`; returns `path` and `url`.
- **POST /api/sync/events** – accepts **menu_item_created** with `event_body.image_path` and **menu_item_image** with `event_body.item_id` and `event_body.image_path`; updates `pos_menu_items.image_path`.
- **GET /api/sync/menu** and **apply_menu** – include `image_path` in categories and items so devices and the portal show images.
- **GET /uploads/{*path}** – serves files under `UPLOAD_DIR` (e.g. `menu/xxx.jpg`).

No cloud changes are required for “sync from POS”; the POS only needs to implement the upload + event flow described in **POS_ITEM_IMAGE_PROMPT.md**.
