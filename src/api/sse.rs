use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::State,
    response::{sse::Event, Sse},
};
use serde::Serialize;
use tokio_stream::StreamExt;

use super::{AppState, StatusResponse, IpInfo};
use crate::api::handlers::ConfigResponse;

#[derive(Serialize)]
struct StreamUpdate {
    status: StatusResponse,
    config: ConfigResponse,
}

pub async fn stream_updates(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(1)))
        .map(move |_| {
            let service = &state.service;
            let running = service.is_running();
            
            let (primary_count, primary_target, backup_count, backup_target, primary_ips, backup_ips, next_health_check, sticky_ips) = 
                if let Some(lb) = service.get_loadbalancer() {
                    let primary_backends = lb.get_primary_backends();
                    let backup_backends = lb.get_backup_backends();
                    
                    let primary_ips: Vec<IpInfo> = primary_backends
                        .iter()
                        .map(IpInfo::from_backend)
                        .collect();
                    
                    let backup_ips: Vec<IpInfo> = backup_backends
                        .iter()
                        .map(IpInfo::from_backend)
                        .collect();
                    
                    let sticky_ips: Vec<String> = lb.get_sticky_ips()
                        .into_iter()
                        .map(|ip| ip.to_string())
                        .collect();
                    
                    (
                        lb.get_primary_count(),
                        lb.get_primary_target(),
                        lb.get_backup_count(),
                        lb.get_backup_target(),
                        primary_ips,
                        backup_ips,
                        lb.get_next_health_check_secs(),
                        sticky_ips,
                    )
                } else {
                    (0, 0, 0, 0, vec![], vec![], 0, vec![])
                };
            
            let config = service.get_config();
            
            let status = StatusResponse {
                running,
                next_health_check,
                health_check_interval: crate::core::config::get_global_config().health_check_interval.as_secs(),
                primary_count,
                primary_target,
                backup_count,
                backup_target,
                sticky_ips,
                primary_ips,
                backup_ips,
            };
            
            let config_resp = ConfigResponse {
                addr: config.listen_addr,
                delay_limit: config.delay_limit,
                tlr: config.tlr,
                ips: config.ips,
                threads: config.threads,
                tls_port: config.tls_port,
                http_port: config.http_port,
                colo: config.colo,
                http: config.http,
                ip_file: config.ip_file,
            };
            
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