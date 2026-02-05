# Event consumer and read model

The cloud receives events from the POS via **POST /api/sync/events** (idempotent by `device_id` + `event_id`). For each new row in `device_event_log`, the event consumer dispatches by `event_type` and upserts/deletes into read-model tables. All ids are POS local strings (e.g. `local_order_id`, `item_id`, `category_id`) so the portal can reference them for commands.

## Event types → read model

| Event type | Action | Read-model table(s) |
|------------|--------|---------------------|
| **Store** | | |
| `store_updated` | Upsert | `pos_store_sync` (store_id, name, timezone) |
| **Menu — categories** | | |
| `menu_category_created` | Upsert | `pos_menus`, `pos_menu_categories` (category_id, menu_id, name, position) |
| `menu_category_renamed` | Update name | `pos_menu_categories` |
| `menu_category_image` | Update image_path | `pos_menu_categories` |
| **Menu — items** | | |
| `menu_item_created` | Upsert | `pos_menu_items` (item_id, store_id, category_id, name, description, price, active, image_path, customer_editable) |
| `menu_item_deleted` | Delete | `pos_menu_items` |
| `menu_item_visibility` | Update active | `pos_menu_items` |
| `menu_item_image` | Update image_path | `pos_menu_items` |
| `menu_item_modifiers_set` | Replace all modifiers for item | `pos_menu_item_modifiers` (delete then insert by position) |
| **Dish yields** | | |
| `dish_yield_upserted` | Upsert | `pos_dish_yields` (menu_item_id, estimated_total, remaining, warning_threshold) |
| `dish_yield_adjusted` | Update remaining | `pos_dish_yields` |
| **Orders/payments** | | |
| `order_created` | Upsert | `orders`, `order_items`, `order_events`. Body: `order_id` (required), `total_cents`/`total`, and `items` or `line_items` (array). Each item: `quantity`/`qty`, `unit_price_cents`/`price_pence`/`unit_price`/`price`, `line_total_cents`/`line_total`, `id`/`item_id`/`local_item_id`, `product_ref`/`product_id`/`menu_item_id`/`name`/`product_name`. |
| `transaction_completed` | Upsert | `transactions`, `order_events`. Body: `order_id`, `transaction_id`/`local_transaction_id`, `amount_cents`/`amount`, `kind`/`payment_method`/`provider`. |
| `receipt_created` | Upsert | `receipts`, `order_events`. Body: `order_id`, `receipt_id`/`local_receipt_id`, `transaction_id`/`local_transaction_id`. |

## Read-model tables (POS local ids)

- **pos_store_sync** — `device_id`, `local_store_id`, name, timezone  
- **pos_menus** — `device_id`, `local_menu_id`  
- **pos_menu_categories** — `device_id`, `local_menu_id`, `local_category_id`, name, position, image_path  
- **pos_menu_items** — `device_id`, `local_item_id`, local_store_id, local_category_id, name, description, price_pence, active, image_path, customer_editable  
- **pos_menu_item_modifiers** — `device_id`, `local_menu_item_id`, name, price_delta_pence, position  
- **pos_dish_yields** — `device_id`, `local_menu_item_id`, estimated_total, remaining, warning_threshold  
- **orders** — `local_order_id`, total_cents, status, occurred_at (plus org_id, store_id, device_id)  
- **order_items** — order_id (cloud), local_item_id, product_ref (menu_item_id), quantity, unit_price_cents  
- **transactions** — local_transaction_id, order_id (cloud), kind, amount_cents  
- **receipts** — local_receipt_id, order_id (cloud), transaction_id (cloud)  

## Activation keys and commands

- **Activation key issuance:** Use **POST /api/admin/activation-keys** (or `scripts/create-activation-key.sh`) to create org/store and issue a key. Store SHA-256 in `device_activation_keys`; show the raw key once so POS users can paste it in Settings → Cloud.
- **Commands:** When sending `void_order` or `refund_order`, set `command_body.local_order_id` (or `order_id`) to the POS order id string from the read model (`orders.local_order_id`).

Full event payload details: see POS repo `docs/CLOUD_PROJECT_BUILD_PROMPT.md`.
