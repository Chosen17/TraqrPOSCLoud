use axum::{
    extract::State,
    http::{header::SET_COOKIE, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

use crate::session::CurrentUser;
use crate::state::AppState;
use db::{
    add_org_membership, add_store_membership, create_cloud_user, create_organization,
    create_session, create_store, get_org_id_by_slug, get_traqr_internal_role, get_profile,
    slug_from_title, update_last_login, verify_login,
};
use domain::{LoginRequest, LoginResponse, SignupRequest};

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

pub fn router(_state: AppState) -> axum::Router<AppState> {
    Router::new()
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

async fn signup(
    State(state): State<AppState>,
    Json(req): Json<SignupRequest>,
) -> Result<Response, (StatusCode, Json<LoginResponse>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        err_response(StatusCode::SERVICE_UNAVAILABLE, "Service unavailable")
    })?;

    // Basic validation
    let business_name = req.business_name.trim();
    let store_name = req.store_name.trim();
    let email = req.email.trim();
    let password = req.password.as_str();
    if business_name.is_empty() || store_name.is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "Business name and store name are required",
        ));
    }
    if email.is_empty() || !email.contains('@') {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "A valid email address is required",
        ));
    }
    if password.len() < 8 {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters",
        ));
    }

    // Ensure email not already taken.
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM cloud_users WHERE LOWER(email) = LOWER(?) LIMIT 1",
    )
    .bind(email)
    .fetch_optional(db)
    .await
    .map_err(|e| {
        tracing::error!("signup email lookup error: {}", e);
        err_response(StatusCode::INTERNAL_SERVER_ERROR, "Sign-up error")
    })?;
    if existing.is_some() {
        return Err(err_response(
            StatusCode::CONFLICT,
            "An account with this email already exists",
        ));
    }

    // Create user
    let display_name = business_name;
    let user_id = create_cloud_user(db, email, password, Some(display_name))
        .await
        .map_err(|e| {
            tracing::error!("create_cloud_user error: {}", e);
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Sign-up error")
        })?;

    // Create organization with unique slug
    let mut slug = slug_from_title(business_name);
    if slug.is_empty() {
        slug = "org".to_string();
    }
    let mut suffix = 1;
    loop {
        if let Ok(opt) = get_org_id_by_slug(db, &slug).await {
            if opt.is_none() {
                break;
            }
        } else {
            break;
        }
        suffix += 1;
        slug = format!("{}-{}", slug_from_title(business_name), suffix);
    }
    let org_id = create_organization(db, business_name, &slug)
        .await
        .map_err(|e| {
            tracing::error!("create_organization error: {}", e);
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Sign-up error")
        })?;

    // Create initial store
    let store_id = create_store(db, org_id, store_name, None)
        .await
        .map_err(|e| {
            tracing::error!("create_store error: {}", e);
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Sign-up error")
        })?;

    // Org + store memberships for the creator
    if let Err(e) = add_org_membership(db, org_id, &user_id, "head_office_admin").await {
        tracing::error!("add_org_membership error: {}", e);
    }
    if let Err(e) = add_store_membership(db, org_id, store_id, &user_id, "store_manager").await {
        tracing::error!("add_store_membership error: {}", e);
    }

    // Create session
    let (_, token) =
        create_session(db, &user_id, SESSION_TTL_SECS).await.map_err(|e| {
            tracing::error!("create_session (signup) error: {}", e);
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Sign-up error")
        })?;

    let body = LoginResponse {
        ok: true,
        message: "Account created".to_string(),
        display_name: Some(display_name.to_string()),
        user_id: Some(user_id.clone()),
        role: None,
    };

    let cookie = format!(
        "{}={}; Path=/; HttpOnly; Max-Age={}; SameSite=Lax",
        SESSION_COOKIE_NAME, token, SESSION_TTL_SECS
    );

    let mut res = (StatusCode::CREATED, Json(body)).into_response();
    res.headers_mut().insert(
        SET_COOKIE,
        cookie
            .parse()
            .unwrap_or_else(|_| panic!("invalid cookie")),
    );
    Ok(res)
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
