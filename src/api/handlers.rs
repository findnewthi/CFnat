use axum::{
    extract::State,
    Json,
};

use super::{AppState, ApiResponse, StartRequest, StatusResponse, StatusInfo, ServerConfig};
use crate::core::config::get_global_config;
use crate::log::get_log_buffer;

pub async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
    let service = &state.service;
    let running = service.is_running();
    let uptime_secs = service.get_uptime_secs();
    
    let info = if let Some(lb) = service.get_loadbalancer() {
        StatusInfo::from_loadbalancer(&lb)
    } else {
        StatusInfo::empty()
    };
    
    Json(StatusResponse {
        running,
        uptime_secs,
        next_health_check: info.next_health_check,
        health_check_interval: get_global_config().health_check_interval.as_secs(),
        primary_count: info.primary_count,
        primary_target: info.primary_target,
        backup_count: info.backup_count,
        backup_target: info.backup_target,
        sticky_ips: info.sticky_ips,
        primary_ips: info.primary_ips,
        backup_ips: info.backup_ips,
    })
}

pub async fn get_config(State(state): State<AppState>) -> Json<ServerConfig> {
    Json(ServerConfig::from(state.service.get_config()))
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
    if let Some(max_sticky_slots) = req.max_sticky_slots {
        config.max_sticky_slots = max_sticky_slots;
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

pub async fn get_logs() -> Json<Vec<crate::log::LogEntry>> {
    Json(get_log_buffer().get_all())
}

pub async fn clear_logs() -> Json<ApiResponse> {
    get_log_buffer().clear();
    Json(ApiResponse {
        success: true,
        message: "日志已清空".to_string(),
    })
}