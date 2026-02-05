# Prompt: Menu item images — give to the POS project

Copy the section below into the other project’s spec or into an AI assistant so it implements item image sync with Traqr Cloud correctly.

---

## Copy from here (prompt for POS / other project)

**Traqr Cloud — menu item images**

When a user adds or changes an image for a menu item on the POS, the image must be uploaded to the cloud first; then we send the cloud’s path in a sync event. The cloud does not accept local file paths — it must receive the actual file.

**1. Upload the image**

- **Endpoint:** `POST {CLOUD_API_URL}/sync/upload-item-image`
- **Headers:** `Authorization: Bearer <device_token>` (same token from activation).
- **Body:** `multipart/form-data` with one file part. The part should be named `file` or `image` (the cloud also accepts any part that has a filename). Max file size 5MB. Allowed types: jpg, jpeg, png, gif, webp.
- **Success (200):** JSON `{ "url": "/uploads/menu/abc-123.jpg", "path": "menu/abc-123.jpg" }`. Use the **`path`** value in step 2 (do not use `url` in events).

**2. Send the path in a sync event**

- **New item (create with image):** When sending **menu_item_created** via `POST /api/sync/events`, include in `event_body`:  
  `"image_path": "<value of path from upload response>"`  
  Example: `"image_path": "menu/abc-123.jpg"`.
- **Existing item (user just set or changed the image):** Send a separate event in the same sync batch (or a later one):  
  **event_type:** `menu_item_image`  
  **event_body:** `{ "item_id": "<our local item id>", "image_path": "<value of path from upload response>" }`  
  Example: `{ "item_id": "item-xyz", "image_path": "menu/abc-123.jpg" }`.

**3. Order of operations**

- When the user saves an item **with an image** (new or updated):  
  1) Upload the image file to `POST .../sync/upload-item-image`.  
  2) If upload fails, show an error and do not sync the item image (or retry).  
  3) If upload succeeds, take `response.path` and:  
     - For a **new** item: include `image_path: response.path` in the **menu_item_created** event body when you POST /api/sync/events.  
     - For an **existing** item: send a **menu_item_image** event with `item_id` (our local id) and `image_path: response.path`.  
  4) Then send the event(s) in the usual way (POST /api/sync/events with `events` array).

**4. What not to do**

- Do **not** put a local path (e.g. `file:///...` or a path on the device) in `image_path` and skip the upload. The cloud will store the string but will have no file to serve, so the image will not appear in the portal or on other devices.
- Do **not** use the `url` from the upload response inside sync events; use **`path`** only.

**5. Summary**

| User action | POS action |
|-------------|------------|
| Creates new menu item with image | 1) POST file to /sync/upload-item-image. 2) Send menu_item_created with image_path = response.path. |
| Changes image on existing item | 1) POST file to /sync/upload-item-image. 2) Send menu_item_image with item_id and image_path = response.path. |

Base URL: **`https://cloud.traqr.co.uk/api`** (or set `CLOUD_API_URL` to that in the POS). Auth: device token only (Bearer), same as for GET /sync/menu and POST /sync/events.

---

## End of prompt

Use the above as the single source of behaviour for “item image + cloud” in the POS. For full API details (errors, field lists), see the Traqr Cloud repo: `docs/API_CONTRACT.md` (Sync: upload item image, Sync: events).
