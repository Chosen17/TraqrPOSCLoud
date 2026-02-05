//! Resolve session cookie to current user for portal routes.

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::COOKIE, request::Parts, StatusCode},
};
use async_trait::async_trait;

use crate::state::AppState;
use db::get_user_id_by_session_token;

const SESSION_COOKIE_NAME: &str = "traqr_session";

fn token_from_cookie_header(cookie_header: Option<&str>) -> Option<String> {
    let header = cookie_header?;
    for part in header.split(';') {
        let part = part.trim();
        if part.starts_with(SESSION_COOKIE_NAME) && part.as_bytes().get(SESSION_COOKIE_NAME.len()) == Some(&b'=') {
            let value = part[SESSION_COOKIE_NAME.len() + 1..].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extractor that resolves session cookie to user_id. Returns 401 if missing or invalid.
pub struct CurrentUser(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
        let cookie_header = parts.headers.get(COOKIE).and_then(|v| v.to_str().ok());
        let token = token_from_cookie_header(cookie_header).ok_or((StatusCode::UNAUTHORIZED, "not logged in"))?;
        let user_id = get_user_id_by_session_token(db, &token)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed"))?;
        let user_id = user_id.ok_or((StatusCode::UNAUTHORIZED, "invalid or expired session"))?;
        Ok(CurrentUser(user_id))
    }
}

/// Optional current user (for routes that work with or without login).
pub struct OptionalUser(pub Option<String>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let db = match &state.db {
            Some(pool) => pool,
            None => return Ok(OptionalUser(None)),
        };
        let cookie_header = parts.headers.get(COOKIE).and_then(|v| v.to_str().ok());
        let token = match token_from_cookie_header(cookie_header) {
            Some(t) => t,
            None => return Ok(OptionalUser(None)),
        };
        let user_id = get_user_id_by_session_token(db, &token).await.ok().flatten();
        Ok(OptionalUser(user_id))
    }
}
