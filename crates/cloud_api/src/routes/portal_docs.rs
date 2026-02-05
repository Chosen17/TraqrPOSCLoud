//! Docs: public read by slug; owner/manager can CRUD. Stripe-style sidebar + breadcrumbs.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::session::CurrentUser;
use crate::state::AppState;
use db::{
    create_doc, delete_doc, get_doc_by_id, get_doc_by_slug, get_traqr_internal_role, list_docs,
    slug_from_title, update_doc,
};

fn can_manage_docs(role: Option<&str>) -> bool {
    matches!(role, Some("sa_owner") | Some("sa_manager"))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/portal/docs", get(list_docs_handler))
        .route("/portal/docs/public", get(list_public_docs))
        .route("/portal/docs/by-slug/:slug", get(get_doc_by_slug_handler))
        .route("/portal/docs", post(create_doc_handler))
        .route("/portal/docs/:id", get(get_doc_handler))
        .route("/portal/docs/:id", post(update_doc_handler))
        .route("/portal/docs/:id", delete(delete_doc_handler))
        .with_state(state)
}

#[derive(Debug, serde::Serialize)]
pub struct DocOut {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub section: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

fn doc_to_out(row: &db::DocRow) -> DocOut {
    DocOut {
        id: row.id.clone(),
        title: row.title.clone(),
        slug: row.slug.clone(),
        body: row.body.clone(),
        section: row.section.clone(),
        sort_order: row.sort_order,
        created_at: row.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
        updated_at: row.updated_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
    }
}

async fn list_docs_handler(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<DocOut>>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_docs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can list docs"));
    }
    let rows = list_docs(db).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "list failed"))?;
    Ok(Json(rows.iter().map(doc_to_out).collect()))
}

async fn list_public_docs(
    State(state): State<AppState>,
) -> Result<Json<Vec<DocOut>>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let rows = list_docs(db).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "list failed"))?;
    Ok(Json(rows.iter().map(doc_to_out).collect()))
}

async fn get_doc_by_slug_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DocOut>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let row = get_doc_by_slug(db, &slug).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "lookup failed"))?;
    let row = row.ok_or((StatusCode::NOT_FOUND, "doc not found"))?;
    Ok(Json(doc_to_out(&row)))
}

#[derive(Debug, Deserialize)]
pub struct CreateDocBody {
    pub title: String,
    pub slug: Option<String>,
    pub body: String,
    pub section: Option<String>,
    pub sort_order: Option<i32>,
}

async fn create_doc_handler(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateDocBody>,
) -> Result<(StatusCode, Json<DocOut>), (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_docs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can create docs"));
    }
    let slug = body.slug.unwrap_or_else(|| slug_from_title(&body.title));
    let section = body.section.as_deref().unwrap_or("General").to_string();
    let sort_order = body.sort_order.unwrap_or(0);
    let id = create_doc(db, &body.title, &slug, &body.body, &section, sort_order)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "create failed"))?;
    let row = get_doc_by_id(db, &id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "fetch failed"))?.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "doc not found"))?;
    Ok((StatusCode::CREATED, Json(doc_to_out(&row))))
}

async fn get_doc_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DocOut>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let row = get_doc_by_id(db, &id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "lookup failed"))?;
    let row = row.ok_or((StatusCode::NOT_FOUND, "doc not found"))?;
    Ok(Json(doc_to_out(&row)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateDocBody {
    pub title: String,
    pub slug: Option<String>,
    pub body: String,
    pub section: Option<String>,
    pub sort_order: Option<i32>,
}

async fn update_doc_handler(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateDocBody>,
) -> Result<Json<DocOut>, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_docs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can edit docs"));
    }
    let slug = body.slug.unwrap_or_else(|| slug_from_title(&body.title));
    let section = body.section.as_deref().unwrap_or("General").to_string();
    let sort_order = body.sort_order.unwrap_or(0);
    update_doc(db, &id, &body.title, &slug, &body.body, &section, sort_order)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "update failed"))?;
    let row = get_doc_by_id(db, &id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "fetch failed"))?.ok_or((StatusCode::NOT_FOUND, "doc not found"))?;
    Ok(Json(doc_to_out(&row)))
}

async fn delete_doc_handler(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let db = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "database unavailable"))?;
    let role = get_traqr_internal_role(db, &user.0).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "role lookup failed"))?;
    if !can_manage_docs(role.as_deref()) {
        return Err((StatusCode::FORBIDDEN, "only owner and manager can delete docs"));
    }
    delete_doc(db, &id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "delete failed"))?;
    Ok(StatusCode::NO_CONTENT)
}
