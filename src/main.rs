use std::sync::Arc;

use axum::{
    Router,
    routing::get,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod code_agent;
mod config;
mod db;
mod fixer;
mod health;
mod llm;
mod models;
mod watcher;

use config::AppConfig;
use db::Database;

/// Shared application state
pub struct AppState {
    pub config: AppConfig,
    pub db: Database,
    pub http: reqwest::Client,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "muninn=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();
    let addr = format!("0.0.0.0:{}", config.port);

    let db = Database::new(&config.database_path)
        .expect("Failed to initialize database");
    db.migrate().expect("Failed to run database migrations");

    let http = reqwest::Client::new();

    let state = Arc::new(AppState { config: config.clone(), db, http });

    // Start GitHub issue watcher in background
    let watcher_state = state.clone();
    tokio::spawn(async move {
        watcher::start_watcher(watcher_state).await;
    });

    let app = Router::new()
        .route("/health", get(health::health_check))
        .route("/healthz", get(health::health_check))
        .route("/api/issues", get(watcher::api::list_issues))
        .route("/api/issues/{id}", get(watcher::api::get_issue))
        .route("/api/issues/{id}/fix", axum::routing::post(watcher::api::trigger_fix))
        .route("/api/issues/{id}/approve", axum::routing::post(watcher::api::approve_fix))
        .route("/api/issues/{id}/reject", axum::routing::post(watcher::api::reject_fix))
        .route("/api/stats", get(watcher::api::get_stats))
        .route("/api/config", get(watcher::api::get_config))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    tracing::info!("🐦 Muninn starting on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
