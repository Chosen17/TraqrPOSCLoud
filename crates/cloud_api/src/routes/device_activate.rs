use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use uuid::Uuid;

use crate::state::AppState;
use domain::{ActivateDeviceRequest, ActivateDeviceResponse};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/device/activate", post(activate_device))
        .with_state(state)
}

async fn activate_device(
    State(_state): State<AppState>,
    Json(req): Json<ActivateDeviceRequest>,
) -> Json<ActivateDeviceResponse> {
    // v1 stub:
    // - later: validate activation key, entitlements, scope/store assignment
    // - later: create device row + issue device_token (hashed in DB)
    //
    // For now, just return a token so the API flow can be tested end-to-end.

    let device_id = Uuid::new_v4();
    let org_id = Uuid::new_v4();
    let store_id = req.store_hint.unwrap_or_else(Uuid::new_v4);

    Json(ActivateDeviceResponse {
        device_id,
        org_id,
        store_id,
        device_token: format!("devtok_{}", Uuid::new_v4()),
        polling_interval_seconds: 10,
    })
}
