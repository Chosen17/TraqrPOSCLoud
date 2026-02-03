use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::state::AppState;
use db::{ack_command, fetch_deliverable_commands, mark_command_delivered, validate_device_token};
use domain::{CommandAckRequest, DeviceCommandOut, SyncCommandsResponse};

pub fn router(_state: AppState) -> axum::Router<AppState> {
    Router::new()
        .route("/sync/commands", get(get_commands))
        .route("/sync/commands/ack", post(ack_command_handler))
}

fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Deserialize)]
pub struct CommandsQuery {
    pub limit: Option<u32>,
}

async fn get_commands(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<CommandsQuery>,
) -> Result<Json<SyncCommandsResponse>, (StatusCode, String)> {
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

    let limit = q.limit.unwrap_or(50).min(200) as i64;
    let rows = fetch_deliverable_commands(db, identity.device_id, limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let commands: Vec<DeviceCommandOut> = rows
        .into_iter()
        .map(|r| DeviceCommandOut {
            command_id: r.command_id,
            command_type: r.command_type,
            sensitive: r.sensitive,
            command_body: r.command_body,
        })
        .collect();

    for cmd in &commands {
        let _ = mark_command_delivered(db, cmd.command_id).await;
    }

    Ok(Json(SyncCommandsResponse { commands }))
}

async fn ack_command_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CommandAckRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
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

    let status = match req.status.as_str() {
        "acked" | "failed" => req.status.as_str(),
        _ => return Err((StatusCode::BAD_REQUEST, "status must be 'acked' or 'failed'".to_string())),
    };

    let updated = ack_command(
        db,
        identity.device_id,
        req.command_id,
        status,
        req.result.as_ref(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !updated {
        return Err((
            StatusCode::NOT_FOUND,
            "command not found or already acked/failed".to_string(),
        ));
    }

    Ok(StatusCode::OK)
}
