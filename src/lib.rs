use std::sync::Arc;

use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod cache;
pub mod config;
pub mod db;
pub mod errors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod services;

pub use cache::AppCache;
pub use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub cache: AppCache,
}

/// Build the Axum router with all middleware. Used by the binary and integration tests.
pub fn build_app(state: AppState) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    routes::create_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}

pub async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    eprintln!(">> Starting task-manager-api...");

    let config = Config::from_env()?;
    eprintln!(">> DATABASE_URL = {}", config.database_url);

    eprintln!(">> Connecting to database...");
    let pool = db::create_pool(&config.database_url).await?;
    eprintln!(">> Connected to database!");

    eprintln!(">> Running migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    eprintln!(">> Migrations complete.");

    let state = AppState {
        db: pool,
        config: Arc::new(config),
        cache: AppCache::new(),
    };

    let app = build_app(state);

    let addr = "0.0.0.0:8080";
    eprintln!(">> Server running at http://{}", addr);
    eprintln!(">> Swagger UI: http://{}/swagger-ui/", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
