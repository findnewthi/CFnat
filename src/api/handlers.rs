use std::net::SocketAddr;

use axum::{
    extract::State,
    Json,
};

use super::{AppState, ApiResponse, IpInfo, StartRequest, StatusResponse};
use crate::core::config::get_global_config;

#[derive(serde::Serialize)]
pub struct ConfigResponse {
    pub addr: SocketAddr,
    pub delay_limit: u64,
    pub tlr: f64,
    pub ips: usize,
    pub threads: usize,
    pub tls_port: u16,
    pub http_port: u16,
    pub colo: Option<Vec<String>>,
    pub http: String,
    pub ip_file: String,
}

pub async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
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
    
    Json(StatusResponse {
        running,
        next_health_check,
        health_check_interval: get_global_config().health_check_interval.as_secs(),
        primary_count,
        primary_target,
        backup_count,
        backup_target,
        sticky_ips,
        primary_ips,
        backup_ips,
    })
}

pub async fn get_config(State(state): State<AppState>) -> Json<ConfigResponse> {
    let config = state.service.get_config();
    
    Json(ConfigResponse {
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
    })
}

pub async fn start_service(
    State(state): State<AppState>,
    Json(req): Json<StartRequest>,
) -> Json<ApiResponse> {
    let mut config = state.service.get_config();
    
    if let Some(ip_file) = req.ip_file {
        config.ip_file = ip_file;
    }
    if let Some(http) = req.http {
        config.http = http;
    }
    if let Some(delay_limit) = req.delay_limit {
        config.delay_limit = delay_limit;
    }
    if let Some(tlr) = req.tlr {
        config.tlr = tlr;
    }
    if let Some(ips) = req.ips {
        config.ips = ips;
    }
    if let Some(threads) = req.threads {
        config.threads = threads;
    }
    if let Some(tls_port) = req.tls_port {
        config.tls_port = tls_port;
    }
    if let Some(http_port) = req.http_port {
        config.http_port = http_port;
    }
    if let Some(colo) = req.colo {
        config.colo = Some(colo);
    }
    if let Some(listen_addr) = req.listen_addr {
        config.listen_addr = listen_addr;
    }
    
    state.service.update_config(config);
    
    match state.service.start() {
        Ok(_) => Json(ApiResponse {
            success: true,
            message: "服务已启动".to_string(),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: e,
        }),
    }
}

pub async fn stop_service(State(state): State<AppState>) -> Json<ApiResponse> {
    match state.service.stop() {
        Ok(_) => Json(ApiResponse {
            success: true,
            message: "服务已停止".to_string(),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: e,
        }),
    }
}

pub async fn health_check() -> Json<ApiResponse> {
    Json(ApiResponse {
        success: true,
        message: "服务运行正常".to_string(),
    })
}