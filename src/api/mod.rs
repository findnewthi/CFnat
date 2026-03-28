mod handlers;
mod routes;
mod sse;

pub use handlers::*;
pub use routes::create_router;
pub use sse::stream_updates;

use serde::{Deserialize, Serialize, Serializer};
use std::net::SocketAddr;
use std::sync::Arc;

#[cfg(feature = "web")]
use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
#[cfg(feature = "web")]
use rust_embed::RustEmbed;

use crate::core::ServiceState;

fn serialize_f64<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64((*value * 100.0).round() / 100.0)
}

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<ServiceState>,
}

#[derive(Clone, Serialize)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub delay_limit: u64,
    #[serde(serialize_with = "serialize_f64")]
    pub tlr: f64,
    pub ips: usize,
    pub threads: usize,
    pub tls_port: u16,
    pub http_port: u16,
    pub colo: Option<Vec<String>>,
    pub http: String,
    pub ip_file: String,
    pub max_sticky_slots: usize,
}

impl From<crate::core::ServiceConfig> for ServerConfig {
    fn from(config: crate::core::ServiceConfig) -> Self {
        Self {
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
            max_sticky_slots: config.max_sticky_slots,
        }
    }
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub running: bool,
    pub uptime_secs: u64,
    pub next_health_check: u64,
    pub health_check_interval: u64,
    pub primary_count: usize,
    pub primary_target: usize,
    pub backup_count: usize,
    pub backup_target: usize,
    pub sticky_ips: Vec<String>,
    pub primary_ips: Vec<IpInfo>,
    pub backup_ips: Vec<IpInfo>,
}

pub struct StatusInfo {
    pub primary_count: usize,
    pub primary_target: usize,
    pub backup_count: usize,
    pub backup_target: usize,
    pub primary_ips: Vec<IpInfo>,
    pub backup_ips: Vec<IpInfo>,
    pub next_health_check: u64,
    pub sticky_ips: Vec<String>,
}

impl StatusInfo {
    pub fn from_loadbalancer(lb: &crate::core::loadbalancer::LoadBalancer) -> Self {
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
        
        Self {
            primary_count: lb.get_primary_count(),
            primary_target: lb.get_primary_target(),
            backup_count: lb.get_backup_count(),
            backup_target: lb.get_backup_target(),
            primary_ips,
            backup_ips,
            next_health_check: lb.get_next_health_check_secs(),
            sticky_ips,
        }
    }
    
    pub fn empty() -> Self {
        Self {
            primary_count: 0,
            primary_target: 0,
            backup_count: 0,
            backup_target: 0,
            primary_ips: vec![],
            backup_ips: vec![],
            next_health_check: 0,
            sticky_ips: vec![],
        }
    }
}

#[derive(Serialize)]
pub struct IpInfo {
    pub ip: String,
    pub colo: Option<String>,
    pub delay: f64,
    pub loss: f64,
    pub samples: usize,
}

impl IpInfo {
    pub fn from_backend(backend: &std::sync::Arc<crate::core::backend::Backend>) -> Self {
        Self {
            ip: backend.addr.ip().to_string(),
            colo: backend.get_colo(),
            delay: backend.get_avg_delay() as f64,
            loss: backend.get_loss_rate() as f64,
            samples: backend.get_sample_count(),
        }
    }
}

#[derive(Deserialize)]
pub struct StartRequest {
    pub ip_file: Option<String>,
    pub http: Option<String>,
    pub delay_limit: Option<u64>,
    pub tlr: Option<f64>,
    pub ips: Option<usize>,
    pub threads: Option<usize>,
    pub tls_port: Option<u16>,
    pub http_port: Option<u16>,
    pub colo: Option<Vec<String>>,
    pub listen_addr: Option<SocketAddr>,
    pub max_sticky_slots: Option<usize>,
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub delay_limit: Option<u64>,
    pub tlr: Option<f64>,
    pub ips: Option<usize>,
}

#[derive(Serialize)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

#[cfg(feature = "web")]
#[derive(RustEmbed)]
#[folder = "flutter/build/web"]
struct Assets;

#[cfg(feature = "web")]
fn get_mime_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(feature = "web")]
pub async fn serve_embedded_files(uri: Uri) -> Response {
    use axum::{
        http::{header, StatusCode},
        response::IntoResponse,
    };
    
    let path = uri.path().trim_start_matches('/');
    
    if path.is_empty() || path == "index.html" {
        return match Assets::get("index.html") {
            Some(content) => (
                [(header::CONTENT_TYPE, "text/html")],
                content.data.into_owned(),
            ).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/html")],
                "<h1>404 - 页面未找到</h1><p>请先构建Flutter Web界面</p>".as_bytes().to_vec(),
            ).into_response(),
        };
    }
    
    match Assets::get(path) {
        Some(content) => {
            let mime = get_mime_type(path);
            ([(header::CONTENT_TYPE, mime)], content.data.into_owned()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}