use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};

use crate::state::AppState;
use db::{update_last_login, verify_login};
use domain::{LoginRequest, LoginResponse};

pub fn router(state: AppState) -> axum::Router<AppState> {
    Router::new().route("/auth/login", post(login))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<LoginResponse>)> {
    let db = match &state.db {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(LoginResponse {
                    ok: false,
                    message: "Service unavailable".to_string(),
                    display_name: None,
                }),
            ));
        }
    };

    let user = verify_login(db, req.email.trim(), &req.password)
        .await
        .map_err(|_e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(LoginResponse {
                    ok: false,
                    message: "Login error".to_string(),
                    display_name: None,
                }),
            )
        })?;

    let Some(user) = user else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                ok: false,
                message: "Invalid email or password".to_string(),
                display_name: None,
            }),
        ));
    };

    let _ = update_last_login(db, user.id).await;

    Ok(Json(LoginResponse {
        ok: true,
        message: "Logged in".to_string(),
        display_name: user.display_name,
    }))
}
