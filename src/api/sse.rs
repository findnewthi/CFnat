use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::State,
    response::{sse::Event, Sse},
};
use serde::Serialize;
use tokio_stream::StreamExt;

use super::{AppState, StatusResponse, StatusInfo, ServerConfig};

#[derive(Serialize)]
struct StreamUpdate {
    status: StatusResponse,
    config: ServerConfig,
}

pub async fn stream_updates(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(1)))
        .map(move |_| {
            let service = &state.service;
            let running = service.is_running();
            let uptime_secs = service.get_uptime_secs();
            
            let info = if let Some(lb) = service.get_loadbalancer() {
                StatusInfo::from_loadbalancer(&lb)
            } else {
                StatusInfo::empty()
            };
            
            let config = service.get_config();
            
            let status = StatusResponse {
                running,
                uptime_secs,
                next_health_check: info.next_health_check,
                health_check_interval: crate::core::config::get_global_config().health_check_interval.as_secs(),
                primary_count: info.primary_count,
                primary_target: info.primary_target,
                backup_count: info.backup_count,
                backup_target: info.backup_target,
                sticky_ips: info.sticky_ips,
                primary_ips: info.primary_ips,
                backup_ips: info.backup_ips,
            };
            
            let config_resp = ServerConfig::from(config);
            
            let update = StreamUpdate {
                status,
                config: config_resp,
            };
            
            Ok(Event::default().json_data(update).unwrap())
        });
    
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
    )
}