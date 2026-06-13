use std::sync::Arc;

use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cache;
mod config;
mod db;
mod errors;
mod handlers;
mod middleware;
mod models;
mod routes;
mod services;

use cache::AppCache;
use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub cache: AppCache,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("FATAL ERROR: {:#}", e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    // Load .env
    dotenvy::dotenv().ok();

    // Init tracing
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

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::create_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr = "0.0.0.0:8080";
    eprintln!(">> Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}