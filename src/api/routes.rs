use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use super::{AppState, serve_embedded_files};
use crate::api::handlers::{get_config, get_status, health_check, start_service, stop_service};
use crate::api::sse::stream_updates;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/config", get(get_config))
        .route("/api/start", post(start_service))
        .route("/api/stop", post(stop_service))
        .route("/api/health", get(health_check))
        .route("/api/stream", get(stream_updates))
        .fallback(serve_embedded_files)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state)
}