use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;
use sqlx::Row;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct DashboardSummary {
    pub total_orders: i64,
    pub today_orders: i64,
    pub total_revenue_cents: i64,
    pub device_count: i64,
    pub last_event_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RecentOrder {
    pub id: String,
    pub local_order_id: String,
    pub store_name: String,
    pub status: String,
    pub total_cents: Option<i64>,
    pub occurred_at: String,
}

#[derive(Debug, Serialize)]
pub struct RecentOrdersResponse {
    pub orders: Vec<RecentOrder>,
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/portal/dashboard/summary", get(get_summary))
        .route("/portal/orders/recent", get(get_recent_orders))
}

async fn get_summary(
    State(state): State<AppState>,
) -> Result<Json<DashboardSummary>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;

    // Total orders
    let total_orders: i64 = sqlx::query("SELECT COUNT(*) AS c FROM orders")
        .fetch_one(db)
        .await
        .map_err(internal)?
        .get::<i64, _>("c");

    // Today's orders (by occurred_at in UTC date)
    let today_orders: i64 = sqlx::query(
        "SELECT COUNT(*) AS c FROM orders WHERE DATE(occurred_at) = CURDATE()",
    )
    .fetch_one(db)
    .await
    .map_err(internal)?
    .get::<i64, _>("c");

    // Total revenue from transactions
    let total_revenue_cents: i64 =
        sqlx::query("SELECT COALESCE(SUM(amount_cents), 0) AS s FROM transactions")
            .fetch_one(db)
            .await
            .map_err(internal)?
            .get::<i64, _>("s");

    // Device count
    let device_count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM devices")
        .fetch_one(db)
        .await
        .map_err(internal)?
        .get::<i64, _>("c");

    // Last event timestamp
    let last_event_at: Option<chrono::NaiveDateTime> =
        sqlx::query("SELECT MAX(received_at) AS m FROM device_event_log")
            .fetch_one(db)
            .await
            .map_err(internal)?
            .get::<Option<chrono::NaiveDateTime>, _>("m");

    Ok(Json(DashboardSummary {
        total_orders,
        today_orders,
        total_revenue_cents,
        device_count,
        last_event_at: last_event_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
    }))
}

async fn get_recent_orders(
    State(state): State<AppState>,
) -> Result<Json<RecentOrdersResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;

    let rows = sqlx::query(
        r#"
        SELECT
          o.id,
          o.local_order_id,
          s.name AS store_name,
          o.status,
          o.total_cents,
          o.occurred_at
        FROM orders o
        JOIN stores s ON s.id = o.store_id
        ORDER BY o.occurred_at DESC
        LIMIT 20
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(internal)?;

    let orders = rows
        .into_iter()
        .map(|row| RecentOrder {
            id: row.get::<String, _>("id"),
            local_order_id: row.get::<String, _>("local_order_id"),
            store_name: row.get::<String, _>("store_name"),
            status: row.get::<String, _>("status"),
            total_cents: row.get::<Option<i64>, _>("total_cents"),
            occurred_at: row
                .get::<chrono::NaiveDateTime, _>("occurred_at")
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
        })
        .collect();

    Ok(Json(RecentOrdersResponse { orders }))
}

fn internal<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

