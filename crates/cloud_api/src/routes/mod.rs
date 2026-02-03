use axum::Router;

use crate::state::AppState;

pub mod auth_login;
pub mod device_activate;
pub mod sync_commands;
pub mod sync_events;

/// Build the application router (public + authenticated + device sync endpoints).
/// Returns Router<AppState>; state is applied once in main via .with_state(state).
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(auth_login::router(state.clone()))
        .merge(device_activate::router(state.clone()))
        .merge(sync_events::router(state.clone()))
        .merge(sync_commands::router(state))
}
