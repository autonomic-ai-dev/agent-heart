use axum::{
    extract::State,
    http::StatusCode,
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
        .route("/outcomes/record", post(outcome_record))
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

type JsonReply = (StatusCode, Json<serde_json::Value>);

fn reply(body: serde_json::Value) -> JsonReply {
    (StatusCode::OK, Json(body))
}

fn failure(error: impl std::fmt::Display) -> JsonReply {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    )
}

async fn budget_stats(State(state): State<Arc<AppState>>) -> JsonReply {
    match token_budget::load_stats(&state.config.token_budget) {
        Ok(report) => reply(serde_json::json!({ "ok": true, "stats": report })),
        Err(e) => failure(e),
    }
}

async fn budget_check(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BudgetCheckRequest>,
) -> JsonReply {
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
            reply(serde_json::json!({ "ok": true, "decision": resp }))
        }
        Err(e) => failure(e),
    }
}

async fn wasm_record(
    State(state): State<Arc<AppState>>,
    Json(rec): Json<WasmExecutionRecord>,
) -> JsonReply {
    match token_budget::record_wasm_execution(&state.config.token_budget, &rec) {
        Ok(()) => reply(serde_json::json!({"ok": true})),
        Err(e) => failure(e),
    }
}

async fn outcome_record(
    State(state): State<Arc<AppState>>,
    Json(outcome): Json<crate::outcomes::ExecutionOutcome>,
) -> JsonReply {
    let path = token_budget::heart_db_path(&state.config.token_budget);
    match crate::outcomes::record(&path, &outcome) {
        Ok(()) => reply(serde_json::json!({"ok": true})),
        Err(e) => failure(e),
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
    async fn outcome_route_stores_valid_report_and_rejects_bad_label() {
        let dir = std::env::temp_dir().join(format!("heart-outcome-route-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("heart.db");
        let mut config = Config::default();
        config.token_budget.heart_db_path = Some(db.display().to_string());
        let state = Arc::new(AppState {
            config,
            spine: SpineClient::new("http://127.0.0.1:3999", "agent-heart", "test"),
        });
        let post = |id: &str| {
            Request::builder()
                .method("POST")
                .uri("/outcomes/record")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": id,
                        "organ": "agent-muscle",
                        "kind": "bash",
                        "success": true,
                        "exit_code": 0,
                        "duration_ms": 5
                    })
                    .to_string(),
                ))
                .unwrap()
        };

        let accepted = router(state.clone()).oneshot(post("job-1")).await.unwrap();
        assert_eq!(accepted.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(accepted.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("\"ok\":true"));
        assert_eq!(crate::outcomes::count(&db).unwrap(), 1);

        let rejected = router(state).oneshot(post("job 1")).await.unwrap();
        assert_eq!(rejected.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(rejected.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("\"ok\":false"));
        assert_eq!(crate::outcomes::count(&db).unwrap(), 1);
    }

    #[tokio::test]
    async fn error_routes_answer_500_with_ok_false() {
        let dir = std::env::temp_dir().join(format!("heart-error-routes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"a file, not a directory").unwrap();
        let mut config = Config::default();
        config.token_budget.heart_db_path = Some(blocker.join("heart.db").display().to_string());
        config.token_budget.brain_db_path = Some(blocker.display().to_string());
        let state = Arc::new(AppState {
            config,
            spine: SpineClient::new("http://127.0.0.1:3999", "agent-heart", "test"),
        });

        let json_post = |uri: &str, body: String| {
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap()
        };
        let requests = [
            Request::builder()
                .method("GET")
                .uri("/budget/stats")
                .body(Body::empty())
                .unwrap(),
            json_post(
                "/budget/check",
                r#"{"phase":"build","estimated_tokens":10}"#.to_string(),
            ),
            json_post(
                "/budget/wasm/record",
                serde_json::json!({
                    "id": "wasm-1",
                    "job_id": null,
                    "organ": "agent-muscle",
                    "fuel_consumed": 5,
                    "memory_peak_bytes": null,
                    "success": true,
                    "latency_ms": 3
                })
                .to_string(),
            ),
            json_post(
                "/outcomes/record",
                serde_json::json!({
                    "id": "job-error",
                    "organ": "agent-muscle",
                    "kind": "bash",
                    "success": true,
                    "exit_code": 0,
                    "duration_ms": 5
                })
                .to_string(),
            ),
        ];

        for request in requests {
            let uri = request.uri().to_string();
            let response = router(state.clone()).oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::INTERNAL_SERVER_ERROR,
                "{uri} must answer 500 when its store is unavailable"
            );
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert!(
                String::from_utf8_lossy(&bytes).contains("\"ok\":false"),
                "{uri} keeps the ok field"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
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
