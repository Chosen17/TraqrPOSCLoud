use axum::Router;

use crate::state::AppState;

pub mod admin_activation_keys;
pub mod auth_login;
pub mod device_activate;
pub mod billing;
pub mod portal_blogs;
pub mod portal_dashboard;
pub mod portal_docs;
pub mod portal_me;
pub mod portal_orgs;
pub mod portal_store;
pub mod portal_orders;
pub mod portal_super_admin;
pub mod delivery_webhooks;
pub mod sync_commands;
pub mod sync_events;
pub mod sync_menu;

/// Build the application router (public + authenticated + device sync endpoints).
/// Returns Router<AppState>; state is applied once in main via .with_state(state).
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(auth_login::router(state.clone()))
        .merge(billing::router(state.clone()))
        .merge(device_activate::router(state.clone()))
        .merge(sync_events::router(state.clone()))
        .merge(sync_commands::router(state.clone()))
        .merge(sync_menu::router(state.clone()))
        .merge(admin_activation_keys::router(state.clone()))
        .merge(portal_dashboard::router(state.clone()))
        .merge(portal_me::router(state.clone()))
        .merge(portal_orgs::router(state.clone()))
        .merge(portal_store::router(state.clone()))
        .merge(portal_orders::router(state.clone()))
        .merge(portal_blogs::router(state.clone()))
        .merge(portal_docs::router(state.clone()))
        .merge(portal_super_admin::router(state.clone()))
        .merge(delivery_webhooks::router(state))
}
