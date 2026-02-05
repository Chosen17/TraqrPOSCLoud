use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use db::{
    has_active_entitlement, insert_event_idempotent, project_event_to_read_model,
    project_event_to_orders, update_device_sync_state_ack_seq, validate_device_token,
};
use domain::{SyncEventsRequest, SyncEventsResponse};

use crate::state::AppState;

pub fn router(_state: AppState) -> axum::Router<AppState> {
    Router::new().route("/sync/events", post(sync_events))
}

fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

async fn sync_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SyncEventsRequest>,
) -> Result<Json<SyncEventsResponse>, (StatusCode, String)> {
    let db = state.db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not available".to_string(),
    ))?;
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "missing or invalid Authorization".to_string()))?;
    let token_hash = hash_token(token);
    let identity = validate_device_token(db, &token_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "invalid or revoked device token".to_string()))?;

    // Enforce Cloud Sync entitlement at org level before accepting events.
    let cloud_sync_ok = has_active_entitlement(db, identity.org_id, "cloud_sync")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !cloud_sync_ok {
        return Err((
            StatusCode::FORBIDDEN,
            serde_json::json!({ "error": "Cloud sync not enabled for this organization" }).to_string(),
        ));
    }
    let mut ack_seq = req.last_ack_seq;

    for e in &req.events {
        let occurred_at = match chrono::DateTime::parse_from_rfc3339(&e.occurred_at) {
            Ok(dt) => dt.with_timezone(&chrono::Utc),
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("invalid occurred_at for event {}: expected RFC3339", e.event_id),
                ));
            }
        };

        let inserted = insert_event_idempotent(
            db,
            identity.org_id,
            identity.store_id,
            identity.device_id,
            e.event_id,
            e.seq,
            &e.event_type,
            &e.event_body,
            occurred_at,
        )
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

        if inserted {
            if let Err(err) = project_event_to_read_model(
                db,
                identity.org_id,
                identity.store_id,
                identity.device_id,
                &e.event_type,
                &e.event_body,
                occurred_at,
            )
            .await
            {
                tracing::warn!("read model projection failed for {}: {}", e.event_type, err);
            }
            let _ = project_event_to_orders(
                db,
                identity.org_id,
                identity.store_id,
                identity.device_id,
                &e.event_type,
                &e.event_body,
                occurred_at,
            )
            .await;
        }

        if let Some(seq) = e.seq {
            ack_seq = Some(match ack_seq {
                Some(curr) => curr.max(seq),
                None => seq,
            });
        }
    }

    update_device_sync_state_ack_seq(
        db,
        identity.device_id,
        identity.org_id,
        identity.store_id,
        ack_seq,
    )
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(Json(SyncEventsResponse { ack_seq }))
}
