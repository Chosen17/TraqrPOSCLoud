use axum::{
    extract::State,
    http::{header::SET_COOKIE, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

use crate::session::CurrentUser;
use crate::state::AppState;
use db::{create_session, get_traqr_internal_role, get_profile, update_last_login, verify_login};
use domain::{LoginRequest, LoginResponse};

const SESSION_COOKIE_NAME: &str = "traqr_session";
const SESSION_TTL_SECS: i64 = 7 * 24 * 3600; // 7 days

fn err_response(status: StatusCode, message: &str) -> (StatusCode, Json<LoginResponse>) {
    (
        status,
        Json(LoginResponse {
            ok: false,
            message: message.to_string(),
            display_name: None,
            user_id: None,
            role: None,
        }),
    )
}

pub fn router(state: AppState) -> axum::Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<LoginResponse>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        err_response(StatusCode::SERVICE_UNAVAILABLE, "Service unavailable")
    })?;

    let user = verify_login(db, req.email.trim(), &req.password).await.map_err(|e| {
        tracing::error!("verify_login error: {}", e);
        err_response(StatusCode::INTERNAL_SERVER_ERROR, "Login error")
    })?;

    let user = user.ok_or_else(|| {
        err_response(StatusCode::UNAUTHORIZED, "Invalid email or password")
    })?;

    let _ = update_last_login(db, &user.id).await;

    let role = get_traqr_internal_role(db, &user.id).await.map_err(|e| {
        tracing::error!("get_traqr_internal_role error: {}", e);
        err_response(StatusCode::INTERNAL_SERVER_ERROR, "Login error")
    })?;

    let (_, token) = create_session(db, &user.id, SESSION_TTL_SECS).await.map_err(|e| {
        tracing::error!("create_session error: {}", e);
        err_response(StatusCode::INTERNAL_SERVER_ERROR, "Login error")
    })?;

    let body = LoginResponse {
        ok: true,
        message: "Logged in".to_string(),
        display_name: user.display_name.clone(),
        user_id: Some(user.id.clone()),
        role,
    };

    let cookie = format!(
        "{}={}; Path=/; HttpOnly; Max-Age={}; SameSite=Lax",
        SESSION_COOKIE_NAME,
        token,
        SESSION_TTL_SECS
    );

    let mut res = (StatusCode::OK, Json(body)).into_response();
    res.headers_mut().insert(
        SET_COOKIE,
        cookie.parse().unwrap_or_else(|_| panic!("invalid cookie")),
    );
    Ok(res)
}

async fn logout() -> Response {
    let cookie = format!("{}=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax", SESSION_COOKIE_NAME);
    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut().insert(
        SET_COOKIE,
        cookie.parse().unwrap_or_else(|_| panic!("invalid cookie")),
    );
    res
}

#[derive(serde::Serialize)]
pub struct MeResponse {
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub profile: MeProfile,
}

#[derive(serde::Serialize)]
pub struct MeProfile {
    pub avatar_path: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub bio: Option<String>,
}

async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<MeResponse>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, email, display_name FROM cloud_users WHERE id = ? AND status = 'active'",
    )
    .bind(&user.0)
    .fetch_optional(db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "user lookup failed"))?;
    let (user_id, email, display_name) = row.ok_or((StatusCode::NOT_FOUND, "user not found"))?;
    let role = get_traqr_internal_role(db, &user_id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    let profile_row = get_profile(db, &user_id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "profile lookup failed"))?;
    let profile = MeProfile {
        avatar_path: profile_row.as_ref().and_then(|p| p.avatar_path.clone()),
        phone: profile_row.as_ref().and_then(|p| p.phone.clone()),
        job_title: profile_row.as_ref().and_then(|p| p.job_title.clone()),
        bio: profile_row.as_ref().and_then(|p| p.bio.clone()),
    };
    Ok(Json(MeResponse {
        user_id,
        email,
        display_name,
        role,
        profile,
    }))
}
