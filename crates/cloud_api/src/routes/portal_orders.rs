use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::session::CurrentUser;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct OrderPathParams {
    pub order_id: String,
}

#[derive(Debug, Serialize)]
pub struct OrderItemRow {
    pub local_item_id: Option<String>,
    pub product_ref: Option<String>,
    pub quantity: f64,
    pub unit_price_cents: Option<i64>,
    pub line_total_cents: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TransactionRow {
    pub id: String,
    pub kind: String,
    pub amount_cents: i64,
    pub occurred_at: String,
}

#[derive(Debug, Serialize)]
pub struct ReceiptRow {
    pub id: String,
    pub local_receipt_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Serialize)]
pub struct OrderDetailResponse {
    pub id: String,
    pub org_id: String,
    pub store_id: String,
    pub device_id: String,
    pub local_order_id: String,
    pub status: String,
    pub total_cents: Option<i64>,
    pub occurred_at: String,
    pub items: Vec<OrderItemRow>,
    pub transactions: Vec<TransactionRow>,
    pub receipts: Vec<ReceiptRow>,
}

#[derive(Debug, Deserialize)]
pub struct EnqueueCommandRequest {
    pub command_type: String, // "void_order" | "refund_order"
}

#[derive(Debug, Serialize)]
pub struct EnqueueCommandResponse {
    pub command_id: String,
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/portal/orders/:order_id", get(get_order_detail))
        .route(
            "/portal/orders/:order_id/commands",
            post(enqueue_order_command),
        )
}

async fn get_order_detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(OrderPathParams { order_id }): Path<OrderPathParams>,
) -> Result<Json<OrderDetailResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let order_uuid =
        Uuid::parse_str(&order_id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid order_id".to_string()))?;

    let order_row = sqlx::query(
        r#"
        SELECT id, org_id, store_id, device_id, local_order_id, status, total_cents, occurred_at
        FROM orders
        WHERE id = ?
        "#,
    )
    .bind(order_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(internal)?;

    let Some(order_row) = order_row else {
        return Err((StatusCode::NOT_FOUND, "order not found".to_string()));
    };

    // Tenant/security: ensure the current user can access the store for this order.
    let store_id: String = order_row.get("store_id");
    let store_uuid =
        Uuid::parse_str(&store_id).map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "invalid store_id".to_string()))?;
    let allowed = db::user_can_access_store(db, &user.0, store_uuid)
        .await
        .map_err(internal)?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let item_rows = sqlx::query(
        r#"
        SELECT
          local_item_id,
          product_ref,
          CAST(quantity AS DOUBLE) AS quantity,
          unit_price_cents,
          line_total_cents
        FROM order_items
        WHERE order_id = ?
        ORDER BY created_at
        "#,
    )
    .bind(order_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let items = item_rows
        .into_iter()
        .map(|row| OrderItemRow {
            local_item_id: row.get::<Option<String>, _>("local_item_id"),
            product_ref: row.get::<Option<String>, _>("product_ref"),
            quantity: row.get::<f64, _>("quantity"),
            unit_price_cents: row.get::<Option<i64>, _>("unit_price_cents"),
            line_total_cents: row.get::<Option<i64>, _>("line_total_cents"),
        })
        .collect();

    let tx_rows = sqlx::query(
        r#"
        SELECT id, kind, amount_cents, occurred_at
        FROM transactions
        WHERE order_id = ?
        ORDER BY occurred_at
        "#,
    )
    .bind(order_uuid.to_string())
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let transactions = tx_rows
        .into_iter()
        .map(|row| TransactionRow {
            id: row.get::<String, _>("id"),
            kind: row.get::<String, _>("kind"),
            amount_cents: row.get::<i64, _>("amount_cents"),
            occurred_at: row
                .get::<chrono::NaiveDateTime, _>("occurred_at")
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
        })
        .collect();

    let device_id: String = order_row.get("device_id");
    let local_order_id: String = order_row.get("local_order_id");
    let rc_rows = sqlx::query(
        r#"
        SELECT id, local_receipt_id, occurred_at
        FROM receipts
        WHERE order_id = ? OR (store_id = ? AND device_id = ? AND local_order_id = ?)
        ORDER BY occurred_at
        "#,
    )
    .bind(order_uuid.to_string())
    .bind(&store_id)
    .bind(&device_id)
    .bind(&local_order_id)
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let receipts = rc_rows
        .into_iter()
        .map(|row| ReceiptRow {
            id: row.get::<String, _>("id"),
            local_receipt_id: row.get::<String, _>("local_receipt_id"),
            occurred_at: row
                .get::<chrono::NaiveDateTime, _>("occurred_at")
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
        })
        .collect();

    Ok(Json(OrderDetailResponse {
        id: order_row.get::<String, _>("id"),
        org_id: order_row.get::<String, _>("org_id"),
        store_id: order_row.get::<String, _>("store_id"),
        device_id: order_row.get::<String, _>("device_id"),
        local_order_id: order_row.get::<String, _>("local_order_id"),
        status: order_row.get::<String, _>("status"),
        total_cents: order_row.get::<Option<i64>, _>("total_cents"),
        occurred_at: order_row
            .get::<chrono::NaiveDateTime, _>("occurred_at")
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
        items,
        transactions,
        receipts,
    }))
}

async fn enqueue_order_command(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(OrderPathParams { order_id }): Path<OrderPathParams>,
    Json(body): Json<EnqueueCommandRequest>,
) -> Result<Json<EnqueueCommandResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let order_uuid =
        Uuid::parse_str(&order_id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid order_id".to_string()))?;

    if body.command_type != "void_order" && body.command_type != "refund_order" {
        return Err((
            StatusCode::BAD_REQUEST,
            "command_type must be 'void_order' or 'refund_order'".to_string(),
        ));
    }

    let order_row = sqlx::query(
        r#"
        SELECT org_id, store_id, device_id, local_order_id
        FROM orders
        WHERE id = ?
        "#,
    )
    .bind(order_uuid.to_string())
    .fetch_optional(db)
    .await
    .map_err(internal)?;

    let Some(order_row) = order_row else {
        return Err((StatusCode::NOT_FOUND, "order not found".to_string()));
    };

    let org_id_s: String = order_row.get("org_id");
    let store_id_s: String = order_row.get("store_id");
    let device_id_s: String = order_row.get("device_id");
    let local_order_id: String = order_row.get("local_order_id");
    let org_id = Uuid::parse_str(&org_id_s).map_err(|_| internal("invalid org_id"))?;
    let store_id = Uuid::parse_str(&store_id_s).map_err(|_| internal("invalid store_id"))?;
    let device_id = Uuid::parse_str(&device_id_s).map_err(|_| internal("invalid device_id"))?;

    let allowed = db::user_can_access_store(db, &user.0, store_id)
        .await
        .map_err(internal)?;
    if !allowed {
        return Err((StatusCode::FORBIDDEN, "store not in your account".to_string()));
    }

    let command_id = Uuid::new_v4();
    let command_body = serde_json::json!({ "local_order_id": local_order_id });

    sqlx::query(
        r#"
        INSERT INTO device_command_queue (
          command_id,
          org_id,
          store_id,
          device_id,
          command_type,
          command_body,
          status,
          sensitive,
          created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, 'queued', 1, CURRENT_TIMESTAMP(3))
        "#,
    )
    .bind(command_id.to_string())
    .bind(org_id.to_string())
    .bind(store_id.to_string())
    .bind(device_id.to_string())
    .bind(&body.command_type)
    .bind(command_body)
    .execute(db)
    .await
    .map_err(internal)?;

    Ok(Json(EnqueueCommandResponse {
        command_id: command_id.to_string(),
    }))
}

fn internal<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

