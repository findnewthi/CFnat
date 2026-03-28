use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use super::AppState;
use crate::api::handlers::{get_config, get_status, health_check, start_service, stop_service, get_logs, clear_logs};
use crate::api::sse::stream_updates;

#[cfg(feature = "web")]
use super::serve_embedded_files;

pub fn create_router(state: AppState) -> Router {
    let router = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/config", get(get_config))
        .route("/api/start", post(start_service))
        .route("/api/stop", post(stop_service))
        .route("/api/health", get(health_check))
        .route("/api/stream", get(stream_updates))
        .route("/api/logs", get(get_logs))
        .route("/api/logs/clear", post(clear_logs))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    #[cfg(feature = "web")]
    let router = router.fallback(serve_embedded_files);

    router
}