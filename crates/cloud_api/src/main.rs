mod routes;
mod session;
mod state;

use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use include_dir::{include_dir, Dir};
use state::AppState;

/// Web UI files embedded at compile time so the app works no matter where it's run from.
static WEB_PUBLIC: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../web/public");

#[tokio::main]
async fn main() {
    // Load .env from workspace root (when running from project root)
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://owlmailer:T5xsfgl3@localhost:3306/traqrcloud".to_string());
    let db = match db::connect(&database_url).await {
        Ok(pool) => {
            if let Err(e) = db::run_migrations(&pool).await {
                tracing::error!("Migrations failed: {}", e);
                tracing::error!("Run from project root with DATABASE_URL set: cargo install sqlx-cli --no-default-features --features mysql && sqlx migrate run");
                return;
            }
            tracing::info!("migrations applied");
            if let Err(e) = db::ensure_plans_table(&pool).await {
                tracing::error!("Critical table missing (migrations may be incomplete): {}", e);
                return;
            }
            tracing::info!("Database: connected");
            Some(pool)
        }
        Err(e) => {
            let url_redacted = redact_password(&database_url);
            tracing::warn!(
                "Database: not available — {} (API will return 503; web UI will still serve)",
                e
            );
            tracing::warn!("DATABASE_URL (redacted): {}", url_redacted);
            tracing::warn!("Check: MySQL running? Database 'traqrcloud' exists? User has access?");
            None
        }
    };
    let state = AppState { db };

    // API routes under /api; state applied once so all handlers see the same AppState.
    let api = Router::new()
        .route("/health", get(health))
        .merge(routes::router(state.clone()))
        .with_state(state);

    let app = Router::new()
        .nest("/api", api)
        .route("/uploads/*path", get(serve_uploads))
        .fallback(serve_web);

    let addr = "0.0.0.0:8080";
    tracing::info!("listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<serde_json::Value> {
    let db_status = if state.db.is_some() {
        "connected"
    } else {
        "disconnected"
    };
    axum::Json(serde_json::json!({ "ok": true, "db": db_status }))
}

/// Redact password in DATABASE_URL for safe logging.
fn redact_password(url: &str) -> String {
    if let Some(at) = url.find('@') {
        if let Some(colon) = url.find("://").map(|i| i + 3).filter(|&i| i < at) {
            if let Some(pw_start) = url[colon..].find(':').map(|j| colon + j + 1) {
                if pw_start < at {
                    return format!("{}***{}", &url[..pw_start], &url[at..]);
                }
            }
        }
    }
    url.to_string()
}

async fn serve_uploads(request: Request) -> Response {
    let path = request.uri().path();
    let path = path.trim_start_matches('/').trim_start_matches("uploads/").trim_start_matches('/');
    let path = path.replace('\\', "/");
    if path.is_empty() || path.contains("..") {
        return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::from("invalid path")).unwrap();
    }
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".to_string());
    let base = std::path::Path::new(&upload_dir);
    let base = if base.is_relative() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join(base)
    } else {
        base.to_path_buf()
    };
    let full = base.join(&path);
    if !full.is_file() {
        return Response::builder().status(StatusCode::NOT_FOUND).body(Body::from("not found")).unwrap();
    }
    match tokio::fs::read(&full).await {
        Ok(data) => {
            let mime = mime_guess::from_path(&full).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(data))
                .unwrap()
        }
        Err(_) => Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::from("read error")).unwrap(),
    }
}

async fn serve_web(request: Request) -> Response {
    let path = request.uri().path();
    // SEO: redirect *.html to clean URL (301)
    if path.ends_with(".html") && path != "/index.html" {
        let clean = path.trim_end_matches(".html");
        if let Ok(loc) = header::HeaderValue::from_str(clean) {
            return Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, loc)
                .body(Body::empty())
                .unwrap();
        }
    }

    // If WEB_ROOT is set, try filesystem first (for local dev without rebuilding)
    if let Ok(web_root) = std::env::var("WEB_ROOT") {
        let raw = request.uri().path().trim_start_matches('/').trim_end_matches('/');
        if raw == "blog" || raw.starts_with("blog/") {
            let blog_path = std::path::Path::new(&web_root).join("blog.html");
            if blog_path.is_file() {
                if let Ok(data) = tokio::fs::read(&blog_path).await {
                    let mime = mime_guess::from_path("blog.html").first_or_octet_stream();
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, mime.as_ref())
                        .body(Body::from(data))
                        .unwrap();
                }
            }
        }
        if raw == "docs" || raw.starts_with("docs/") {
            let docs_path = std::path::Path::new(&web_root).join("docs.html");
            if docs_path.is_file() {
                if let Ok(data) = tokio::fs::read(&docs_path).await {
                    let mime = mime_guess::from_path("docs.html").first_or_octet_stream();
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, mime.as_ref())
                        .body(Body::from(data))
                        .unwrap();
                }
            }
        }
        let path = if raw.is_empty() {
            "index.html".to_string()
        } else if !raw.contains('.') && !raw.ends_with('/') {
            format!("{}.html", raw)
        } else {
            raw.to_string()
        };
        let full = std::path::Path::new(&web_root).join(&path);
        if !full.is_file() && path.ends_with(".html") {
            let alt = std::path::Path::new(&web_root).join(raw).join("index.html");
            if alt.is_file() {
                if let Ok(data) = tokio::fs::read(&alt).await {
                    let mime = mime_guess::from_path(&alt).first_or_octet_stream();
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, mime.as_ref())
                        .body(Body::from(data))
                        .unwrap();
                }
            }
        }
        if full.is_file() {
            if let Ok(data) = tokio::fs::read(&full).await {
                let mime = mime_guess::from_path(&full).first_or_octet_stream();
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime.as_ref())
                    .body(Body::from(data))
                    .unwrap();
            }
        } else if path == "index.html" {
            let idx = std::path::Path::new(&web_root).join("index.html");
            if idx.is_file() {
                if let Ok(data) = tokio::fs::read(&idx).await {
                    let mime = mime_guess::from_path("index.html").first_or_octet_stream();
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, mime.as_ref())
                        .body(Body::from(data))
                        .unwrap();
                }
            }
        }
    }

    // Serve from embedded dir: / -> index.html, /pricing -> pricing.html
    let raw = request.uri().path().trim_start_matches('/').trim_end_matches('/');
    // SPA-style: /blog and /blog/any-slug serve blog.html; /docs and /docs/any-slug serve docs.html
    if raw == "blog" || raw.starts_with("blog/") {
        if let Some(f) = WEB_PUBLIC.get_file("blog.html") {
            let mime = mime_guess::from_path("blog.html").first_or_octet_stream();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(f.contents()))
                .unwrap();
        }
    }
    if raw == "docs" || raw.starts_with("docs/") {
        if let Some(f) = WEB_PUBLIC.get_file("docs.html") {
            let mime = mime_guess::from_path("docs.html").first_or_octet_stream();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(f.contents()))
                .unwrap();
        }
    }
    let paths: Vec<String> = if raw.is_empty() {
        vec!["index.html".to_string()]
    } else {
        vec![
            raw.to_string(),
            format!("{}.html", raw),
            format!("{}/index.html", raw),
            "index.html".to_string(),
        ]
    };

    for p in &paths {
        if let Some(f) = WEB_PUBLIC.get_file(p) {
            let mime = mime_guess::from_path(p).first_or_octet_stream();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(f.contents()))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not found"))
        .unwrap()
}

