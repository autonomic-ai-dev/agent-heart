use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;

use crate::config::Config;
use crate::spine::SpineClient;

pub struct AppState {
    pub config: Config,
    pub spine: SpineClient,
}

pub async fn start_http(config: Config) -> anyhow::Result<()> {
    let spine = SpineClient::new(&config.spine.url, "agent-heart", env!("CARGO_PKG_VERSION"));
    spine.register().await?;
    let spine_clone = spine.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let _ = spine_clone.heartbeat().await;
        }
    });
    let port = config.server.port;
    let state = Arc::new(AppState { config, spine });
    let app = Router::new()
        .route("/health", get(health))
        .route("/gc/status", get(gc_status))
        .with_state(state);
    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("HTTP server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health(State(_): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn gc_status(State(_): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let state_dir = crate::config::state_dir();
    let last_gc = std::fs::read_to_string(state_dir.join("last_gc.txt"))
        .unwrap_or_else(|_| "never".to_string());
    Json(serde_json::json!({
        "last_gc": last_gc,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
