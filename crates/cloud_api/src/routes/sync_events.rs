use axum::{
    extract::State,
    routing::post,
    Json, Router,
};

use crate::state::AppState;
use domain::{SyncEventsRequest, SyncEventsResponse};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/sync/events", post(sync_events))
        .with_state(state)
}

async fn sync_events(
    State(_state): State<AppState>,
    Json(req): Json<SyncEventsRequest>,
) -> Json<SyncEventsResponse> {
    // v1 stub:
    // - later: insert device_event_log idempotently
    // - later: update device_sync_state last_ack_seq
    // - later: update read models (orders, order_events, etc.)
    //
    // For now, acknowledge the highest seq we received (if any).

    let mut ack = req.last_ack_seq;

    for e in &req.events {
        if let Some(seq) = e.seq {
            ack = Some(match ack {
                Some(curr) => curr.max(seq),
                None => seq,
            });
        }
    }

    Json(SyncEventsResponse { ack_seq: ack })
}
