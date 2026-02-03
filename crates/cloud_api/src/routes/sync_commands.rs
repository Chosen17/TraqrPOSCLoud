use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::state::AppState;
use domain::{CommandAckRequest, SyncCommandsResponse};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/sync/commands", get(get_commands))
        .route("/sync/commands/ack", post(ack_command))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct CommandsQuery {
    pub limit: Option<u32>,
}

async fn get_commands(
    State(_state): State<AppState>,
    Query(q): Query<CommandsQuery>,
) -> Json<SyncCommandsResponse> {
    // v1 stub:
    // - later: authenticate device (device_token)
    // - later: fetch deliverable commands from device_command_queue
    // - later: enforce approvals for sensitive commands

    let _limit = q.limit.unwrap_or(50).min(200);

    Json(SyncCommandsResponse { commands: vec![] })
}

async fn ack_command(
    State(_state): State<AppState>,
    Json(_req): Json<CommandAckRequest>,
) -> StatusCode {
    // v1 stub:
    // - later: mark command acked/failed, store ack_result
    // - later: update audit logs

    StatusCode::OK
}
