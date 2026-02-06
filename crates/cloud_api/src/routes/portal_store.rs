use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::session::CurrentUser;
use crate::state::AppState;
use db::{
    create_pos_menu_category, create_pos_menu_item, enqueue_apply_menu_for_store,
    ensure_pos_menu, get_device_id_for_store, update_pos_menu_category_by_id,
    update_pos_menu_category_image_by_id, update_pos_menu_item_by_id,
    update_pos_menu_item_image_by_id,
};

#[derive(Debug, Deserialize)]
pub struct StorePathParams {
    pub store_id: String,
}

#[derive(Debug, Serialize)]
pub struct MenuCategory {
    pub id: String,
    pub name: String,
    pub position: i32,
}

#[derive(Debug, Serialize)]
pub struct MenuItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub price_pence: Option<i64>,
    pub active: bool,
    pub image_path: Option<String>,
    pub remaining: Option<f64>,
    pub estimated_total: Option<f64>,
    pub warning_threshold: Option<f64>,
    pub has_modifiers: bool,
}

#[derive(Debug, Serialize)]
pub struct MenuCategoryItemsResponse {
    pub categories: Vec<MenuCategory>,
    pub items: Vec<MenuItem>,
}

#[derive(Debug, Deserialize)]
pub struct MenuQuery {
    pub category_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StoreOrderRow {
    pub id: String,
    pub local_order_id: String,
    pub status: String,
    pub total_cents: Option<i64>,
    pub occurred_at: String,
}

#[derive(Debug, Serialize)]
pub struct StoreOrdersResponse {
    pub orders: Vec<StoreOrderRow>,
}

#[derive(Debug, Serialize)]
pub struct CommandRow {
    pub command_id: String,
    pub command_type: String,
    pub local_order_id: Option<String>,
    pub status: String,
    pub device_id: String,
    pub created_at: String,
    pub sensitive: bool,
}

#[derive(Debug, Serialize)]
pub struct StoreCommandsResponse {
    pub commands: Vec<CommandRow>,
}

#[derive(Debug, Serialize)]
pub struct StoreMetaResponse {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub canonical_device_id: Option<String>,
    pub canonical_device_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchMenuItemBody {
    pub name: Option<String>,
    pub price_pence: Option<i64>,
    pub description: Option<Option<String>>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PatchMenuCategoryBody {
    pub name: Option<String>,
    pub position: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMenuCategoryBody {
    pub name: String,
    #[serde(default)]
    pub position: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateMenuItemBody {
    pub name: String,
    pub description: Option<String>,
    pub price_pence: Option<i64>,
    /// Cloud category id (UUID) to assign the item to.
    pub category_id: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub customer_editable: bool,
}

fn default_true() -> bool {
    true
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/portal/stores/:store_id/menu",
            get(get_store_menu_and_items),
        )
        .route(
            "/portal/stores/:store_id/menu/categories",
            post(post_create_store_menu_category),
        )
        .route(
            "/portal/stores/:store_id/menu/items",
            post(post_create_store_menu_item),
        )
        .route(
            "/portal/stores/:store_id/menu/items/:item_id",
            patch(patch_store_menu_item),
        )
        .route(
            "/portal/stores/:store_id/menu/items/:item_id/upload-image",
            post(upload_store_menu_item_image),
        )
        .route(
            "/portal/stores/:store_id/menu/categories/:category_id/upload-image",
            post(upload_store_menu_category_image),
        )
        .route(
            "/portal/stores/:store_id/menu/categories/:category_id",
            patch(patch_store_menu_category),
        )
        .route("/portal/stores/:store_id/meta", get(get_store_meta))
        .route("/portal/stores/:store_id/orders", get(get_store_orders))
        .route("/portal/stores/:store_id/commands", get(get_store_commands))
}

async fn get_store_meta(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StorePathParams { store_id }): Path<StorePathParams>,
) -> Result<Json<StoreMetaResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let row = sqlx::query(
        r#"
        SELECT
          s.id,
          s.org_id,
          s.name,
          s.canonical_device_id,
          d.device_name AS canonical_device_name
        FROM stores s
        LEFT JOIN devices d ON d.id = s.canonical_device_id
        WHERE s.id = ?
        "#,
    )
    .bind(store_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(internal)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "store not found".to_string()));
    };

    Ok(Json(StoreMetaResponse {
        id: row.get::<String, _>("id"),
        org_id: row.get::<String, _>("org_id"),
        name: row.get::<String, _>("name"),
        canonical_device_id: row.get::<Option<String>, _>("canonical_device_id"),
        canonical_device_name: row.get::<Option<String>, _>("canonical_device_name"),
    }))
}

async fn get_store_menu_and_items(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StorePathParams { store_id }): Path<StorePathParams>,
    Query(q): Query<MenuQuery>,
) -> Result<Json<MenuCategoryItemsResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    // Derive a device_id for this store from device_sync_state; for v1 we
    // assume a single primary device per store for menu/yield display.
    let device_row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT device_id
        FROM device_sync_state
        WHERE store_id = ?
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(store_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(internal)?;

    let device_id = match device_row.and_then(|(s,)| Uuid::parse_str(&s).ok()) {
        Some(u) => u,
        None => {
            return Ok(Json(MenuCategoryItemsResponse {
                categories: Vec::new(),
                items: Vec::new(),
            }));
        }
    };

    let cat_rows = sqlx::query(
        r#"
        SELECT id, name, position
        FROM pos_menu_categories
        WHERE device_id = ?
        ORDER BY position, name
        "#,
    )
    .bind(device_id.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let categories: Vec<MenuCategory> = cat_rows
        .into_iter()
        .map(|row| MenuCategory {
            id: row.get::<String, _>("id"),
            name: row.get::<String, _>("name"),
            position: row.get::<i32, _>("position"),
        })
        .collect();

    let selected_category_id = q.category_id.or_else(|| categories.first().map(|c| c.id.clone()));

    let mut items: Vec<MenuItem> = Vec::new();
    if let Some(cat_id) = selected_category_id {
        // Resolve cloud category id -> POS local_category_id, then load items in that category.
        let cat_row: Option<(String,)> = sqlx::query_as(
            "SELECT local_category_id FROM pos_menu_categories WHERE id = ? AND device_id = ?",
        )
        .bind(&cat_id)
        .bind(device_id.to_string())
        .fetch_optional(db)
        .await
        .map_err(internal)?;

        if let Some((local_category_id,)) = cat_row {
            let item_rows = sqlx::query(
                r#"
                SELECT
                  i.id,
                  i.local_item_id,
                  i.name,
                  i.description,
                  i.price_pence,
                  i.active,
                  i.image_path,
                  y.estimated_total,
                  y.remaining,
                  y.warning_threshold,
                  EXISTS(
                    SELECT 1
                    FROM pos_menu_item_modifiers m
                    WHERE m.device_id = i.device_id
                      AND m.local_menu_item_id = i.local_item_id
                  ) AS has_modifiers
                FROM pos_menu_items i
                LEFT JOIN pos_dish_yields y
                  ON y.device_id = i.device_id
                 AND y.local_menu_item_id = i.local_item_id
                WHERE i.device_id = ?
                  AND i.local_category_id = ?
                ORDER BY i.name
                "#,
            )
            .bind(device_id.to_string())
            .bind(local_category_id)
            .fetch_all(db)
            .await
            .map_err(internal)?;

            items = item_rows
                .into_iter()
                .map(|row| MenuItem {
                    id: row.get::<String, _>("id"),
                    name: row.get::<String, _>("name"),
                    description: row.get::<Option<String>, _>("description"),
                    price_pence: row.get::<Option<i64>, _>("price_pence"),
                    active: row.get::<bool, _>("active"),
                    image_path: row.get::<Option<String>, _>("image_path"),
                    estimated_total: row.get::<Option<f64>, _>("estimated_total"),
                    remaining: row.get::<Option<f64>, _>("remaining"),
                    warning_threshold: row.get::<Option<f64>, _>("warning_threshold"),
                    has_modifiers: row.get::<bool, _>("has_modifiers"),
                })
                .collect();
        }
    } else if categories.is_empty() {
        // No categories yet: load all items for this device so tea/coffee etc still show.
        let item_rows = sqlx::query(
            r#"
            SELECT
              i.id,
              i.local_item_id,
              i.name,
              i.description,
              i.price_pence,
              i.active,
              i.image_path,
              y.estimated_total,
              y.remaining,
              y.warning_threshold,
              EXISTS(
                SELECT 1
                FROM pos_menu_item_modifiers m
                WHERE m.device_id = i.device_id
                  AND m.local_menu_item_id = i.local_item_id
              ) AS has_modifiers
            FROM pos_menu_items i
            LEFT JOIN pos_dish_yields y
              ON y.device_id = i.device_id
             AND y.local_menu_item_id = i.local_item_id
            WHERE i.device_id = ?
            ORDER BY i.name
            "#,
        )
        .bind(device_id.to_string())
        .fetch_all(db)
        .await
        .map_err(internal)?;

        items = item_rows
            .into_iter()
            .map(|row| MenuItem {
                id: row.get::<String, _>("id"),
                name: row.get::<String, _>("name"),
                description: row.get::<Option<String>, _>("description"),
                price_pence: row.get::<Option<i64>, _>("price_pence"),
                active: row.get::<bool, _>("active"),
                image_path: row.get::<Option<String>, _>("image_path"),
                estimated_total: row.get::<Option<f64>, _>("estimated_total"),
                remaining: row.get::<Option<f64>, _>("remaining"),
                warning_threshold: row.get::<Option<f64>, _>("warning_threshold"),
                has_modifiers: row.get::<bool, _>("has_modifiers"),
            })
            .collect();
    }

    Ok(Json(MenuCategoryItemsResponse { categories, items }))
}

async fn get_store_orders(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StorePathParams { store_id }): Path<StorePathParams>,
) -> Result<Json<StoreOrdersResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let rows = sqlx::query(
        r#"
        SELECT id, local_order_id, status, total_cents, occurred_at
        FROM orders
        WHERE store_id = ?
        ORDER BY occurred_at DESC
        LIMIT 100
        "#,
    )
    .bind(store_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let orders = rows
        .into_iter()
        .map(|row| StoreOrderRow {
            id: row.get::<String, _>("id"),
            local_order_id: row.get::<String, _>("local_order_id"),
            status: row.get::<String, _>("status"),
            total_cents: row.get::<Option<i64>, _>("total_cents"),
            occurred_at: row
                .get::<chrono::NaiveDateTime, _>("occurred_at")
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
        })
        .collect();

    Ok(Json(StoreOrdersResponse { orders }))
}

async fn get_store_commands(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StorePathParams { store_id }): Path<StorePathParams>,
) -> Result<Json<StoreCommandsResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let rows = sqlx::query(
        r#"
        SELECT
          command_id,
          command_type,
          command_body,
          status,
          device_id,
          created_at,
          sensitive
        FROM device_command_queue
        WHERE store_id = ?
        ORDER BY created_at DESC
        LIMIT 100
        "#,
    )
    .bind(store_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let commands = rows
        .into_iter()
        .map(|row| {
            let body: serde_json::Value = row.get::<serde_json::Value, _>("command_body");
            let local_order_id = body
                .get("local_order_id")
                .or_else(|| body.get("order_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            CommandRow {
                command_id: row.get::<String, _>("command_id"),
                command_type: row.get::<String, _>("command_type"),
                local_order_id,
                status: row.get::<String, _>("status"),
                device_id: row.get::<String, _>("device_id"),
                created_at: row
                    .get::<chrono::NaiveDateTime, _>("created_at")
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string(),
                sensitive: row.get::<bool, _>("sensitive"),
            }
        })
        .collect();

    Ok(Json(StoreCommandsResponse { commands }))
}

async fn post_create_store_menu_category(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StorePathParams { store_id }): Path<StorePathParams>,
    Json(body): Json<CreateMenuCategoryBody>,
) -> Result<(StatusCode, Json<MenuCategory>), (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let device_id = match get_device_id_for_store(db, store_uuid).await.map_err(internal)? {
        Some(d) => d,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "No device linked to this store. Activate a device first.".to_string(),
            ))
        }
    };

    let org_row: Option<(String,)> = sqlx::query_as("SELECT org_id FROM stores WHERE id = ?")
        .bind(store_uuid.to_string())
        .fetch_optional(db)
        .await
        .map_err(internal)?;
    let org_id = match org_row {
        Some((id,)) => Uuid::parse_str(&id).map_err(|_| internal("invalid org_id"))?,
        None => return Err((StatusCode::NOT_FOUND, "store not found".to_string())),
    };

    let local_menu_row: Option<(String,)> = sqlx::query_as(
        "SELECT local_menu_id FROM pos_menu_categories WHERE device_id = ? LIMIT 1",
    )
    .bind(device_id.to_string())
    .fetch_optional(db)
    .await
    .map_err(internal)?;
    let local_menu_id = local_menu_row
        .map(|(s,)| s)
        .unwrap_or_else(|| "default".to_string());

    let _ = ensure_pos_menu(db, org_id, device_id, &local_menu_id).await;

    let new_id = create_pos_menu_category(
        db,
        org_id,
        device_id,
        &local_menu_id,
        body.name.trim(),
        body.position,
    )
    .await
    .map_err(internal)?;

    let _ = enqueue_apply_menu_for_store(db, store_uuid).await;

    Ok((
        StatusCode::CREATED,
        Json(MenuCategory {
            id: new_id.to_string(),
            name: body.name.trim().to_string(),
            position: body.position,
        }),
    ))
}

async fn post_create_store_menu_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(StorePathParams { store_id }): Path<StorePathParams>,
    Json(body): Json<CreateMenuItemBody>,
) -> Result<(StatusCode, Json<MenuItem>), (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let device_id = match get_device_id_for_store(db, store_uuid).await.map_err(internal)? {
        Some(d) => d,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "No device linked to this store. Activate a device first.".to_string(),
            ))
        }
    };

    let org_row: Option<(String,)> = sqlx::query_as("SELECT org_id FROM stores WHERE id = ?")
        .bind(store_uuid.to_string())
        .fetch_optional(db)
        .await
        .map_err(internal)?;
    let org_id = match org_row {
        Some((id,)) => Uuid::parse_str(&id).map_err(|_| internal("invalid org_id"))?,
        None => return Err((StatusCode::NOT_FOUND, "store not found".to_string())),
    };

    let local_category_id: Option<String> = match &body.category_id {
        Some(cat_id) => {
            let cat_uuid = Uuid::parse_str(cat_id)
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid category_id".to_string()))?;
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT local_category_id FROM pos_menu_categories WHERE id = ? AND device_id = ?",
            )
            .bind(cat_uuid.to_string())
            .bind(device_id.to_string())
            .fetch_optional(db)
            .await
            .map_err(internal)?;
            match row {
                Some((s,)) => Some(s),
                None => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        "category not found in this store".to_string(),
                    ))
                }
            }
        }
        None => None,
    };

    let local_store_row: Option<(String,)> = sqlx::query_as(
        "SELECT local_store_id FROM pos_store_sync WHERE store_id = ? AND device_id = ? LIMIT 1",
    )
    .bind(store_uuid.to_string())
    .bind(device_id.to_string())
    .fetch_optional(db)
    .await
    .map_err(internal)?;
    let local_store_id: Option<String> =
        local_store_row.map(|(s,)| s).or_else(|| Some(store_uuid.to_string()));

    let local_item_id = format!("cloud-{}", Uuid::new_v4());

    let new_id = create_pos_menu_item(
        db,
        org_id,
        device_id,
        &local_item_id,
        local_store_id.as_deref(),
        local_category_id.as_deref(),
        body.name.trim(),
        body.description.as_deref(),
        body.price_pence,
        body.active,
        None,
        body.customer_editable,
    )
    .await
    .map_err(internal)?;

    let _ = enqueue_apply_menu_for_store(db, store_uuid).await;

    Ok((
        StatusCode::CREATED,
        Json(MenuItem {
            id: new_id.to_string(),
            name: body.name.trim().to_string(),
            description: body.description.clone(),
            price_pence: body.price_pence,
            active: body.active,
            image_path: None,
            remaining: None,
            estimated_total: None,
            warning_threshold: None,
            has_modifiers: false,
        }),
    ))
}

async fn patch_store_menu_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((store_id, item_id)): Path<(String, String)>,
    Json(body): Json<PatchMenuItemBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }
    let item_uuid = Uuid::parse_str(&item_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid item_id".to_string()))?;

    let exists: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1 FROM pos_menu_items i
        JOIN device_sync_state d ON d.device_id = i.device_id AND d.store_id = ?
        WHERE i.id = ?
        "#,
    )
    .bind(store_uuid.to_string())
    .bind(item_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "menu item not found in this store".to_string()));
    }

    update_pos_menu_item_by_id(
        db,
        item_uuid,
        body.name.as_deref(),
        body.price_pence,
        body.description.as_ref().map(|o| o.as_deref()),
        body.active,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let _ = enqueue_apply_menu_for_store(db, store_uuid).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn upload_store_menu_item_image(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((store_id, item_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }
    let item_uuid = Uuid::parse_str(&item_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid item_id".to_string()))?;

    let exists: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1 FROM pos_menu_items i
        JOIN device_sync_state d ON d.device_id = i.device_id AND d.store_id = ?
        WHERE i.id = ?
        "#,
    )
    .bind(store_uuid.to_string())
    .bind(item_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "menu item not found in this store".to_string()));
    }

    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".to_string());
    let base = std::path::Path::new(&upload_dir);
    let base = if base.is_relative() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join(base)
    } else {
        base.to_path_buf()
    };
    let menu_dir = base.join("menu");
    let _ = tokio::fs::create_dir_all(&menu_dir).await;

    let mut ext = "jpg".to_string();
    let mut data = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart".to_string()))?
    {
        let name = field.name().unwrap_or("");
        if name != "file" && name != "image" && field.file_name().is_none() {
            continue;
        }
        if let Some(name) = field.file_name() {
            ext = std::path::Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .filter(|e| matches!(*e, "jpg" | "jpeg" | "png" | "gif" | "webp"))
                .unwrap_or("jpg")
                .to_string();
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, "failed to read file".to_string()))?
            .to_vec();
        if bytes.len() > 5 * 1024 * 1024 {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "file too large (max 5MB)".to_string(),
            ));
        }
        data = Some(bytes);
        break;
    }
    let data = data.ok_or((StatusCode::BAD_REQUEST, "missing file field (file or image)".to_string()))?;

    let filename = format!("{}.{}", uuid::Uuid::new_v4(), ext);
    let path = menu_dir.join(&filename);
    tokio::fs::write(&path, &data)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to save file".to_string()))?;

    let relative_path = format!("menu/{}", filename);
    update_pos_menu_item_image_by_id(db, item_uuid, &relative_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _ = enqueue_apply_menu_for_store(db, store_uuid).await;

    Ok(Json(serde_json::json!({
        "url": format!("/uploads/{}", relative_path),
        "path": relative_path
    })))
}

async fn upload_store_menu_category_image(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((store_id, category_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }
    let category_uuid = Uuid::parse_str(&category_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid category_id".to_string()))?;

    let exists: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1 FROM pos_menu_categories c
        JOIN device_sync_state d ON d.device_id = c.device_id AND d.store_id = ?
        WHERE c.id = ?
        "#,
    )
    .bind(store_uuid.to_string())
    .bind(category_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            "menu category not found in this store".to_string(),
        ));
    }

    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".to_string());
    let base = std::path::Path::new(&upload_dir);
    let base = if base.is_relative() {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(base)
    } else {
        base.to_path_buf()
    };
    let menu_dir = base.join("menu");
    let _ = tokio::fs::create_dir_all(&menu_dir).await;

    let mut ext = "jpg".to_string();
    let mut data = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart".to_string()))?
    {
        let name = field.name().unwrap_or("");
        if name != "file" && name != "image" && field.file_name().is_none() {
            continue;
        }
        if let Some(name) = field.file_name() {
            ext = std::path::Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .filter(|e| matches!(*e, "jpg" | "jpeg" | "png" | "gif" | "webp"))
                .unwrap_or("jpg")
                .to_string();
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, "failed to read file".to_string()))?
            .to_vec();
        if bytes.len() > 5 * 1024 * 1024 {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "file too large (max 5MB)".to_string(),
            ));
        }
        data = Some(bytes);
        break;
    }
    let data = data.ok_or((
        StatusCode::BAD_REQUEST,
        "missing file field (file or image)".to_string(),
    ))?;

    let filename = format!("{}.{}", uuid::Uuid::new_v4(), ext);
    let path = menu_dir.join(&filename);
    tokio::fs::write(&path, &data)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to save file".to_string()))?;

    let relative_path = format!("menu/{}", filename);
    update_pos_menu_category_image_by_id(db, category_uuid, &relative_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _ = enqueue_apply_menu_for_store(db, store_uuid).await;

    Ok(Json(serde_json::json!({
        "url": format!("/uploads/{}", relative_path),
        "path": relative_path
    })))
}

async fn patch_store_menu_category(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((store_id, category_id)): Path<(String, String)>,
    Json(body): Json<PatchMenuCategoryBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let store_uuid = Uuid::parse_str(&store_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid store_id".to_string()))?;

    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }
    let category_uuid = Uuid::parse_str(&category_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid category_id".to_string()))?;

    let exists: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1 FROM pos_menu_categories c
        JOIN device_sync_state d ON d.device_id = c.device_id AND d.store_id = ?
        WHERE c.id = ?
        "#,
    )
    .bind(store_uuid.to_string())
    .bind(category_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "menu category not found in this store".to_string()));
    }

    update_pos_menu_category_by_id(db, category_uuid, body.name.as_deref(), body.position)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let _ = enqueue_apply_menu_for_store(db, store_uuid).await;
    Ok(StatusCode::NO_CONTENT)
}

fn internal<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

