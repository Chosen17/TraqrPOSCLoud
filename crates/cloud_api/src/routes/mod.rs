use axum::Router;

use crate::state::AppState;

pub mod device_activate;
pub mod sync_commands;
pub mod sync_events;

/// Build the application router (public + authenticated + device sync endpoints).
pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(device_activate::router(state.clone()))
        .merge(sync_events::router(state.clone()))
        .merge(sync_commands::router(state))
}
