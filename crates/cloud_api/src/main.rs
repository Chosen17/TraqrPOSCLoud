mod routes;
mod state;

use axum::routing::get;
use axum::Router;
use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Temporary: connect later; for now use a lazy placeholder pool setup in file 23+
    // We'll wire DB properly once device endpoints need it.
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/traqr_cloud".to_string());
    let pool = db::connect(&database_url).await.expect("db connect failed");

    let state = AppState { db: pool };

    let app = Router::new()
        .route("/health", get(health))
        .merge(routes::router(state));

    let addr = "0.0.0.0:8080";
    tracing::info!("listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}
