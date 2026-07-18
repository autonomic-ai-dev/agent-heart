use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;

use crate::config::Config;
use crate::spine::SpineClient;
use crate::token_budget::{self, BudgetCheckRequest, WasmExecutionRecord};

pub struct AppState {
    pub config: Config,
    pub spine: SpineClient,
}

pub fn health_json(token_budget_enabled: bool) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "service": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "token_budget_enabled": token_budget_enabled,
    })
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/gc/status", get(gc_status))
        .route("/budget/stats", get(budget_stats))
        .route("/budget/check", post(budget_check))
        .route("/budget/wasm/record", post(wasm_record))
        .with_state(state)
}

pub async fn start_http(config: Config) -> anyhow::Result<()> {
    let spine = SpineClient::new(&config.spine.url, "agent-heart", env!("CARGO_PKG_VERSION"));
    if let Err(e) = spine.register().await {
        tracing::warn!(error = %e, "Failed to register with agent-spine, continuing without registration");
    }
    let spine_clone = spine.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let _ = spine_clone.heartbeat().await;
        }
    });
    let port = config.server.port;
    let state = Arc::new(AppState { config, spine });
    let app = router(state);
    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("HTTP server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(health_json(state.config.token_budget.enabled))
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

async fn budget_stats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match token_budget::load_stats(&state.config.token_budget) {
        Ok(report) => Json(serde_json::json!({ "ok": true, "stats": report })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

async fn budget_check(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BudgetCheckRequest>,
) -> Json<serde_json::Value> {
    match token_budget::check_budget(&state.config.token_budget, &req) {
        Ok(resp) => {
            if resp.frozen {
                let _ = state
                    .spine
                    .publish(
                        "heart.budget.frozen",
                        &serde_json::json!({
                            "phase": req.phase,
                            "estimated_tokens": req.estimated_tokens,
                            "reason": resp.reason,
                        }),
                    )
                    .await;
            }
            Json(serde_json::json!({ "ok": true, "decision": resp }))
        }
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

async fn wasm_record(
    State(state): State<Arc<AppState>>,
    Json(rec): Json<WasmExecutionRecord>,
) -> Json<serde_json::Value> {
    match token_budget::record_wasm_execution(&state.config.token_budget, &rec) {
        Ok(()) => Json(serde_json::json!({"ok": true})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string() })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            config: Config::default(),
            spine: SpineClient::new("http://127.0.0.1:3999", "agent-heart", "test"),
        })
    }

    #[test]
    fn health_json_matches_spec() {
        let body = health_json(true);
        assert_eq!(body["ok"], true);
        assert_eq!(body["service"], "agent-heart");
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(body["token_budget_enabled"], true);
    }

    #[tokio::test]
    async fn health_route_returns_200() {
        let app = router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
