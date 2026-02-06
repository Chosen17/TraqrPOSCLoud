//! Read model: project device_event_log into store, menu, categories, items, modifiers, dish yields.
//! All ids are POS local strings (store_id, category_id, item_id, etc.) for reference in commands.

use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use crate::device::{is_device_canonical_for_store, update_device_name_primary};
use crate::sync::insert_device_config_alert;

// ---------- Store sync ----------

/// Pick a device_id that has synced for this store (for canonical menu). If a
/// canonical_device_id is set on the store, that device is used; otherwise we
/// fall back to the most recently synced device. Returns None if no device has
/// synced yet.
pub async fn get_device_id_for_store(pool: &MySqlPool, store_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    // Prefer an explicit canonical device if the store has one configured.
    if let Some(canonical_id) = crate::device::get_canonical_device_for_store(pool, store_id).await? {
        return Ok(Some(canonical_id));
    }

    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT device_id FROM device_sync_state WHERE store_id = ? ORDER BY updated_at DESC LIMIT 1
        "#,
    )
    .bind(store_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(s,)| Uuid::parse_str(&s).ok()))
}

/// Categories and items for sync (GET /api/sync/menu). Uses same shape as read model.
#[derive(Debug, serde::Serialize)]
pub struct SyncMenuCategory {
    pub local_category_id: String,
    pub local_menu_id: String,
    pub name: String,
    pub position: i32,
    pub image_path: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct SyncMenuItem {
    pub local_item_id: String,
    pub local_store_id: Option<String>,
    pub local_category_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub price_pence: Option<i64>,
    pub active: bool,
    pub image_path: Option<String>,
    pub customer_editable: bool,
}

pub async fn get_store_menu_for_sync(
    pool: &MySqlPool,
    store_id: Uuid,
) -> Result<Option<(Vec<SyncMenuCategory>, Vec<SyncMenuItem>)>, sqlx::Error> {
    let device_id = match get_device_id_for_store(pool, store_id).await? {
        Some(d) => d,
        None => return Ok(None),
    };

    let cat_rows = sqlx::query(
        r#"
        SELECT local_category_id, local_menu_id, name, position, image_path
        FROM pos_menu_categories WHERE device_id = ? ORDER BY position, name
        "#,
    )
    .bind(device_id.to_string())
    .fetch_all(pool)
    .await?;

    let categories: Vec<SyncMenuCategory> = cat_rows
        .into_iter()
        .map(|row| SyncMenuCategory {
            local_category_id: row.get::<String, _>("local_category_id"),
            local_menu_id: row.get::<String, _>("local_menu_id"),
            name: row.get::<String, _>("name"),
            position: row.get::<i32, _>("position"),
            image_path: row.get::<Option<String>, _>("image_path"),
        })
        .collect();

    let item_rows = sqlx::query(
        r#"
        SELECT local_item_id, local_store_id, local_category_id, name, description, price_pence, active, image_path, customer_editable
        FROM pos_menu_items WHERE device_id = ? ORDER BY name
        "#,
    )
    .bind(device_id.to_string())
    .fetch_all(pool)
    .await?;

    let items: Vec<SyncMenuItem> = item_rows
        .into_iter()
        .map(|row| SyncMenuItem {
            local_item_id: row.get::<String, _>("local_item_id"),
            local_store_id: row.get::<Option<String>, _>("local_store_id"),
            local_category_id: row.get::<Option<String>, _>("local_category_id"),
            name: row.get::<String, _>("name"),
            description: row.get::<Option<String>, _>("description"),
            price_pence: row.get::<Option<i64>, _>("price_pence"),
            active: row.get::<bool, _>("active"),
            image_path: row.get::<Option<String>, _>("image_path"),
            customer_editable: row.get::<bool, _>("customer_editable"),
        })
        .collect();

    Ok(Some((categories, items)))
}

pub async fn upsert_pos_store(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    device_id: Uuid,
    local_store_id: &str,
    name: &str,
    timezone: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO pos_store_sync (org_id, store_id, device_id, local_store_id, name, timezone)
        VALUES (?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE name = VALUES(name), timezone = VALUES(timezone)
        "#,
    )
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(local_store_id)
    .bind(name)
    .bind(timezone)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------- Menus ----------

pub async fn ensure_pos_menu(
    pool: &MySqlPool,
    org_id: Uuid,
    device_id: Uuid,
    local_menu_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT IGNORE INTO pos_menus (org_id, device_id, local_menu_id)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(org_id.to_string())
    .bind(device_id.to_string())
    .bind(local_menu_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------- Menu categories ----------

/// Create a new menu category from the cloud (portal). Returns the new row id.
pub async fn create_pos_menu_category(
    pool: &MySqlPool,
    org_id: Uuid,
    device_id: Uuid,
    local_menu_id: &str,
    name: &str,
    position: i32,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let local_category_id = format!("cloud-{}", Uuid::new_v4());
    sqlx::query(
        r#"
        INSERT INTO pos_menu_categories (id, org_id, device_id, local_menu_id, local_category_id, name, position, image_path)
        VALUES (?, ?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(id.to_string())
    .bind(org_id.to_string())
    .bind(device_id.to_string())
    .bind(local_menu_id)
    .bind(&local_category_id)
    .bind(name)
    .bind(position)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn upsert_pos_menu_category(
    pool: &MySqlPool,
    org_id: Uuid,
    device_id: Uuid,
    local_menu_id: &str,
    local_category_id: &str,
    name: &str,
    position: i32,
    image_path: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO pos_menu_categories (org_id, device_id, local_menu_id, local_category_id, name, position, image_path)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE name = VALUES(name), position = VALUES(position), image_path = COALESCE(VALUES(image_path), image_path)
        "#,
    )
    .bind(org_id.to_string())
    .bind(device_id.to_string())
    .bind(local_menu_id)
    .bind(local_category_id)
    .bind(name)
    .bind(position)
    .bind(image_path)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------- Menu items ----------

/// Create a new menu item from the cloud (portal). Returns the new row id.
pub async fn create_pos_menu_item(
    pool: &MySqlPool,
    org_id: Uuid,
    device_id: Uuid,
    local_item_id: &str,
    local_store_id: Option<&str>,
    local_category_id: Option<&str>,
    name: &str,
    description: Option<&str>,
    price_pence: Option<i64>,
    active: bool,
    image_path: Option<&str>,
    customer_editable: bool,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO pos_menu_items (id, org_id, device_id, local_item_id, local_store_id, local_category_id, name, description, price_pence, active, image_path, customer_editable)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(org_id.to_string())
    .bind(device_id.to_string())
    .bind(local_item_id)
    .bind(local_store_id)
    .bind(local_category_id)
    .bind(name)
    .bind(description)
    .bind(price_pence)
    .bind(active)
    .bind(image_path)
    .bind(customer_editable)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn upsert_pos_menu_item(
    pool: &MySqlPool,
    org_id: Uuid,
    device_id: Uuid,
    local_item_id: &str,
    local_store_id: Option<&str>,
    local_category_id: Option<&str>,
    name: &str,
    description: Option<&str>,
    price_pence: Option<i64>,
    active: bool,
    image_path: Option<&str>,
    customer_editable: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO pos_menu_items (org_id, device_id, local_item_id, local_store_id, local_category_id, name, description, price_pence, active, image_path, customer_editable)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
          local_store_id = COALESCE(VALUES(local_store_id), local_store_id),
          local_category_id = COALESCE(VALUES(local_category_id), local_category_id),
          name = VALUES(name),
          description = COALESCE(VALUES(description), description),
          price_pence = COALESCE(VALUES(price_pence), price_pence),
          active = VALUES(active),
          image_path = COALESCE(VALUES(image_path), image_path),
          customer_editable = VALUES(customer_editable)
        "#,
    )
    .bind(org_id.to_string())
    .bind(device_id.to_string())
    .bind(local_item_id)
    .bind(local_store_id)
    .bind(local_category_id)
    .bind(name)
    .bind(description)
    .bind(price_pence)
    .bind(active)
    .bind(image_path)
    .bind(customer_editable)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_pos_menu_category_image(
    pool: &MySqlPool,
    device_id: Uuid,
    local_category_id: &str,
    image_path: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE pos_menu_categories SET image_path = ? WHERE device_id = ? AND local_category_id = ?",
    )
    .bind(image_path)
    .bind(device_id.to_string())
    .bind(local_category_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_pos_menu_item(
    pool: &MySqlPool,
    device_id: Uuid,
    local_item_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM pos_menu_items WHERE device_id = ? AND local_item_id = ?",
    )
    .bind(device_id.to_string())
    .bind(local_item_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_pos_menu_item_active(
    pool: &MySqlPool,
    device_id: Uuid,
    local_item_id: &str,
    active: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE pos_menu_items SET active = ? WHERE device_id = ? AND local_item_id = ?",
    )
    .bind(active)
    .bind(device_id.to_string())
    .bind(local_item_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update menu item by cloud row id (for portal edits). Omitted fields are not changed.
pub async fn update_pos_menu_item_by_id(
    pool: &MySqlPool,
    item_id: Uuid,
    name: Option<&str>,
    price_pence: Option<i64>,
    description: Option<Option<&str>>,
    active: Option<bool>,
) -> Result<bool, sqlx::Error> {
    let mut sets = vec!["updated_at = CURRENT_TIMESTAMP(3)"];
    if name.is_some() {
        sets.push("name = ?");
    }
    if price_pence.is_some() {
        sets.push("price_pence = ?");
    }
    if description.is_some() {
        sets.push("description = ?");
    }
    if active.is_some() {
        sets.push("active = ?");
    }
    if sets.len() == 1 {
        return Ok(false);
    }
    let q = format!("UPDATE pos_menu_items SET {} WHERE id = ?", sets.join(", "));
    let mut query = sqlx::query(&q);
    if let Some(n) = name {
        query = query.bind(n);
    }
    if let Some(p) = price_pence {
        query = query.bind(p);
    }
    if description.is_some() {
        query = query.bind(description.and_then(|o| o.map(|s| s.to_string())));
    }
    if let Some(a) = active {
        query = query.bind(a);
    }
    query = query.bind(item_id.to_string());
    let res = query.execute(pool).await?;
    Ok(res.rows_affected() > 0)
}

/// Update menu category by cloud row id (for portal edits).
pub async fn update_pos_menu_category_by_id(
    pool: &MySqlPool,
    category_id: Uuid,
    name: Option<&str>,
    position: Option<i32>,
) -> Result<bool, sqlx::Error> {
    let mut sets = vec!["updated_at = CURRENT_TIMESTAMP(3)"];
    if name.is_some() {
        sets.push("name = ?");
    }
    if position.is_some() {
        sets.push("position = ?");
    }
    if sets.len() == 1 {
        return Ok(false);
    }
    let q = format!("UPDATE pos_menu_categories SET {} WHERE id = ?", sets.join(", "));
    let mut query = sqlx::query(&q);
    if let Some(n) = name {
        query = query.bind(n);
    }
    if let Some(p) = position {
        query = query.bind(p);
    }
    query = query.bind(category_id.to_string());
    let res = query.execute(pool).await?;
    Ok(res.rows_affected() > 0)
}

pub async fn update_pos_menu_item_image(
    pool: &MySqlPool,
    device_id: Uuid,
    local_item_id: &str,
    image_path: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE pos_menu_items SET image_path = ? WHERE device_id = ? AND local_item_id = ?",
    )
    .bind(image_path)
    .bind(device_id)
    .bind(local_item_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update menu item image by cloud row id (for portal uploads).
pub async fn update_pos_menu_item_image_by_id(
    pool: &MySqlPool,
    item_id: Uuid,
    image_path: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("UPDATE pos_menu_items SET image_path = ?, updated_at = CURRENT_TIMESTAMP(3) WHERE id = ?")
        .bind(image_path)
        .bind(item_id.to_string())
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// Update menu category image by cloud row id (for portal uploads).
pub async fn update_pos_menu_category_image_by_id(
    pool: &MySqlPool,
    category_id: Uuid,
    image_path: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE pos_menu_categories SET image_path = ?, updated_at = CURRENT_TIMESTAMP(3) WHERE id = ?",
    )
    .bind(image_path)
    .bind(category_id.to_string())
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

// ---------- Menu item modifiers (replace all for item) ----------

pub async fn delete_pos_menu_item_modifiers(
    pool: &MySqlPool,
    device_id: Uuid,
    local_menu_item_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM pos_menu_item_modifiers WHERE device_id = ? AND local_menu_item_id = ?",
    )
    .bind(device_id.to_string())
    .bind(local_menu_item_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_pos_menu_item_modifier(
    pool: &MySqlPool,
    device_id: Uuid,
    local_menu_item_id: &str,
    name: &str,
    price_delta_pence: i32,
    position: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO pos_menu_item_modifiers (device_id, local_menu_item_id, name, price_delta_pence, position)
        VALUES (?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE name = VALUES(name), price_delta_pence = VALUES(price_delta_pence)
        "#,
    )
    .bind(device_id.to_string())
    .bind(local_menu_item_id)
    .bind(name)
    .bind(price_delta_pence)
    .bind(position)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------- Dish yields ----------

pub async fn upsert_pos_dish_yield(
    pool: &MySqlPool,
    device_id: Uuid,
    local_menu_item_id: &str,
    estimated_total: Option<f64>,
    remaining: Option<f64>,
    warning_threshold: Option<f64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO pos_dish_yields (device_id, local_menu_item_id, estimated_total, remaining, warning_threshold)
        VALUES (?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
          estimated_total = COALESCE(VALUES(estimated_total), estimated_total),
          remaining = VALUES(remaining),
          warning_threshold = COALESCE(VALUES(warning_threshold), warning_threshold)
        "#,
    )
    .bind(device_id.to_string())
    .bind(local_menu_item_id)
    .bind(estimated_total)
    .bind(remaining)
    .bind(warning_threshold)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn adjust_pos_dish_yield_remaining(
    pool: &MySqlPool,
    device_id: Uuid,
    local_menu_item_id: &str,
    remaining: Option<f64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE pos_dish_yields SET remaining = ? WHERE device_id = ? AND local_menu_item_id = ?
        "#,
    )
    .bind(remaining)
    .bind(device_id.to_string())
    .bind(local_menu_item_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Dispatch by event_type and upsert/delete into store, menu, categories, items, modifiers, yields.
/// Order/payment events (order_created, transaction_completed, receipt_created) are handled by orders::project_event_to_orders.
pub async fn project_event_to_read_model(
    pool: &MySqlPool,
    org_id: Uuid,
    store_id: Uuid,
    device_id: Uuid,
    event_type: &str,
    event_body: &serde_json::Value,
    _occurred_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    // Enforce that menu and configuration updates originate from the canonical
    // device for the store. If a non-canonical device attempts to change these,
    // we skip the projection, log a warning, and record an alert row so the
    // portal can surface it later.
    let is_config_event = matches!(
        event_type,
        "store_updated"
            | "menu_category_created"
            | "menu_category_renamed"
            | "menu_category_image"
            | "menu_item_created"
            | "menu_item_deleted"
            | "menu_item_visibility"
            | "menu_item_image"
            | "menu_item_modifiers_set"
            | "dish_yield_upserted"
            | "dish_yield_adjusted"
    );

    if is_config_event {
        let is_canonical = is_device_canonical_for_store(pool, store_id, device_id).await?;
        if !is_canonical {
            let details = serde_json::to_string(event_body).ok();
            tracing::warn!(
                "non-canonical device attempted config update: org_id={}, store_id={}, device_id={}, event_type={}",
                org_id,
                store_id,
                device_id,
                event_type
            );
            let _ = insert_device_config_alert(
                pool,
                org_id,
                store_id,
                device_id,
                event_type,
                details.as_deref(),
            )
            .await;
            return Ok(());
        }
    }

    match event_type {
        "store_updated" => {
            let local_store_id = event_body.get("store_id").and_then(|v| v.as_str()).unwrap_or("");
            let name = event_body.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let timezone = event_body.get("timezone").and_then(|v| v.as_str()).unwrap_or("Europe/London");
            if !local_store_id.is_empty() {
                upsert_pos_store(pool, org_id, store_id, device_id, local_store_id, name, timezone).await?;
            }
        }
        "menu_category_created" => {
            let local_menu_id = event_body
                .get("menu_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("default");
            let local_category_id = event_body.get("category_id").and_then(|v| v.as_str()).unwrap_or("");
            let name = event_body.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let position = event_body.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            if !local_category_id.is_empty() {
                let _ = ensure_pos_menu(pool, org_id, device_id, local_menu_id).await;
                upsert_pos_menu_category(pool, org_id, device_id, local_menu_id, local_category_id, name, position, None).await?;
            }
        }
        "menu_category_renamed" => {
            let local_category_id = event_body.get("category_id").and_then(|v| v.as_str()).unwrap_or("");
            let name = event_body.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if !local_category_id.is_empty() {
                // Upsert with same category_id; we need local_menu_id - get from existing row or use empty and rely on ON DUPLICATE
                let row: Option<(String,)> = sqlx::query_as(
                    "SELECT local_menu_id FROM pos_menu_categories WHERE device_id = ? AND local_category_id = ?",
                )
                .bind(device_id.to_string())
                .bind(local_category_id)
                .fetch_optional(pool)
                .await?;
                if let Some((local_menu_id,)) = row {
                    upsert_pos_menu_category(pool, org_id, device_id, &local_menu_id, local_category_id, name, 0, None).await?;
                }
            }
        }
        "menu_category_image" => {
            let local_category_id = event_body.get("category_id").and_then(|v| v.as_str()).unwrap_or("");
            let image_path = event_body.get("image_path").and_then(|v| v.as_str()).unwrap_or("");
            if !local_category_id.is_empty() {
                update_pos_menu_category_image(pool, device_id, local_category_id, image_path).await?;
            }
        }
        "menu_item_created" => {
            let local_item_id = event_body.get("item_id").and_then(|v| v.as_str()).unwrap_or("");
            let local_store_id = event_body.get("store_id").and_then(|v| v.as_str());
            let local_category_id = event_body.get("category_id").and_then(|v| v.as_str());
            let name = event_body.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let description = event_body.get("description").and_then(|v| v.as_str());
            let price_pence = event_body
                .get("price_pence")
                .and_then(|v| v.as_i64())
                .or_else(|| event_body.get("price").and_then(|v| v.as_i64()))
                .or_else(|| event_body.get("price").and_then(|v| v.as_f64()).map(|p| (p * 100.0) as i64));
            let active = event_body.get("active").and_then(|v| v.as_bool()).unwrap_or(true);
            let image_path = event_body.get("image_path").and_then(|v| v.as_str());
            let customer_editable = event_body.get("customer_editable").and_then(|v| v.as_bool()).unwrap_or(false);
            if !local_item_id.is_empty() {
                upsert_pos_menu_item(
                    pool,
                    org_id,
                    device_id,
                    local_item_id,
                    local_store_id,
                    local_category_id,
                    name,
                    description,
                    price_pence,
                    active,
                    image_path,
                    customer_editable,
                )
                .await?;
            }
        }
        "menu_item_deleted" => {
            let local_item_id = event_body.get("item_id").and_then(|v| v.as_str()).unwrap_or("");
            if !local_item_id.is_empty() {
                delete_pos_menu_item(pool, device_id, local_item_id).await?;
            }
        }
        "menu_item_visibility" => {
            let local_item_id = event_body.get("item_id").and_then(|v| v.as_str()).unwrap_or("");
            let active = event_body.get("active").and_then(|v| v.as_bool()).unwrap_or(true);
            if !local_item_id.is_empty() {
                update_pos_menu_item_active(pool, device_id, local_item_id, active).await?;
            }
        }
        "menu_item_image" => {
            let local_item_id = event_body.get("item_id").and_then(|v| v.as_str()).unwrap_or("");
            let image_path = event_body.get("image_path").and_then(|v| v.as_str()).unwrap_or("");
            if !local_item_id.is_empty() {
                update_pos_menu_item_image(pool, device_id, local_item_id, image_path).await?;
            }
        }
        "menu_item_modifiers_set" => {
            let local_menu_item_id = event_body.get("menu_item_id").and_then(|v| v.as_str()).unwrap_or("");
            if local_menu_item_id.is_empty() {
                return Ok(());
            }
            delete_pos_menu_item_modifiers(pool, device_id, local_menu_item_id).await?;
            if let Some(modifiers) = event_body.get("modifiers").and_then(|v| v.as_array()) {
                for (idx, m) in modifiers.iter().enumerate() {
                    let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let price_delta_pence = m.get("price_delta_pence").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let position = m.get("position").and_then(|v| v.as_i64()).unwrap_or(idx as i64) as i32;
                    let _ = insert_pos_menu_item_modifier(
                        pool,
                        device_id,
                        local_menu_item_id,
                        name,
                        price_delta_pence,
                        position,
                    )
                    .await;
                }
            }
        }
        "dish_yield_upserted" => {
            let local_menu_item_id = event_body.get("menu_item_id").and_then(|v| v.as_str()).unwrap_or("");
            let estimated_total = event_body.get("estimated_total").and_then(|v| v.as_f64());
            let remaining = event_body.get("remaining").and_then(|v| v.as_f64());
            let warning_threshold = event_body.get("warning_threshold").and_then(|v| v.as_f64());
            if !local_menu_item_id.is_empty() {
                upsert_pos_dish_yield(pool, device_id, local_menu_item_id, estimated_total, remaining, warning_threshold).await?;
            }
        }
        "dish_yield_adjusted" => {
            let local_menu_item_id = event_body.get("menu_item_id").and_then(|v| v.as_str()).unwrap_or("");
            let remaining = event_body.get("remaining").and_then(|v| v.as_f64());
            if !local_menu_item_id.is_empty() {
                adjust_pos_dish_yield_remaining(pool, device_id, local_menu_item_id, remaining).await?;
            }
        }
        "device_updated" => {
            let device_name = event_body.get("device_name").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
            let is_primary = event_body.get("is_primary").and_then(|v| v.as_bool()).unwrap_or(false);
            update_device_name_primary(pool, device_id, device_name, is_primary).await?;
        }
        _ => {}
    }
    Ok(())
}
