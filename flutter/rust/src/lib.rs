mod frb_generated; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
use flutter_rust_bridge::frb;
use std::sync::OnceLock;
use std::sync::Arc;
use cfnat::core::ServiceState;
use cfnat::core::config::get_global_config;

static SERVICE: OnceLock<Arc<ServiceState>> = OnceLock::new();

fn get_service() -> Arc<ServiceState> {
    SERVICE.get_or_init(|| Arc::new(ServiceState::new())).clone()
}

#[frb]
pub struct StatusInfo {
    pub running: bool,
    pub uptime_secs: u64,
    pub next_health_check: u64,
    pub health_check_interval: u64,
    pub primary_count: i32,
    pub primary_target: i32,
    pub backup_count: i32,
    pub backup_target: i32,
    pub primary_ips: Vec<IpInfo>,
    pub backup_ips: Vec<IpInfo>,
    pub sticky_ips: Vec<String>,
}

#[frb]
pub struct IpInfo {
    pub ip: String,
    pub delay: f64,
    pub loss: f64,
    pub samples: i32,
    pub colo: Option<String>,
}

#[frb]
pub struct ConfigInfo {
    pub ip_file: String,
    pub http: String,
    pub delay_limit: u64,
    pub tlr: f64,
    pub ips: i32,
    pub threads: i32,
    pub tls_port: i32,
    pub http_port: i32,
    pub max_sticky_slots: i32,
    pub listen_addr: String,
}

#[frb]
pub struct LogItem {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[frb]
#[tokio::main(flavor = "current_thread")]
pub async fn start_service(
    ip_file: Option<String>,
    ip_content: Option<Vec<String>>,
    http: Option<String>,
    delay_limit: Option<u64>,
    tlr: Option<f64>,
    ips: Option<i32>,
    threads: Option<i32>,
    tls_port: Option<i32>,
    http_port: Option<i32>,
    max_sticky_slots: Option<i32>,
    listen_addr: Option<String>,
) -> bool {
    let service = get_service();
    let mut config = service.get_config();
    
    if let Some(v) = http { config.http = v; }
    if let Some(v) = delay_limit { config.delay_limit = v; }
    if let Some(v) = tlr { config.tlr = v; }
    if let Some(v) = ips { config.ips = v as usize; }
    if let Some(v) = threads { config.threads = v as usize; }
    if let Some(v) = tls_port { config.tls_port = v as u16; }
    if let Some(v) = http_port { config.http_port = v as u16; }
    if let Some(v) = max_sticky_slots { config.max_sticky_slots = v as usize; }
    if let Some(v) = listen_addr {
        if let Ok(addr) = v.parse() {
            config.listen_addr = addr;
        }
    }
    
    service.update_config(config);
    
    service.start_with_ips(
        ip_file.as_deref(),
        ip_content.as_deref(),
    ).is_ok()
}

#[frb]
pub fn stop_service() -> bool {
    let service = get_service();
    service.stop().is_ok()
}

#[frb]
pub fn get_status() -> StatusInfo {
    let service = get_service();
    let running = service.is_running();
    let uptime_secs = service.get_uptime_secs();
    let health_check_interval = get_global_config().health_check_interval.as_secs();
    
    if let Some(lb) = service.get_loadbalancer() {
        let primary_backends = lb.get_primary_backends();
        let backup_backends = lb.get_backup_backends();
        let sticky_ips = lb.get_sticky_ips();
        
        StatusInfo {
            running,
            uptime_secs,
            next_health_check: lb.get_next_health_check_secs(),
            health_check_interval,
            primary_count: primary_backends.len() as i32,
            primary_target: lb.get_primary_target() as i32,
            backup_count: backup_backends.len() as i32,
            backup_target: lb.get_backup_target() as i32,
            primary_ips: primary_backends.iter().map(|b| IpInfo {
                ip: b.addr.to_string(),
                delay: b.get_avg_delay() as f64,
                loss: b.get_loss_rate() as f64,
                samples: b.get_sample_count() as i32,
                colo: b.get_colo(),
            }).collect(),
            backup_ips: backup_backends.iter().map(|b| IpInfo {
                ip: b.addr.to_string(),
                delay: b.get_avg_delay() as f64,
                loss: b.get_loss_rate() as f64,
                samples: b.get_sample_count() as i32,
                colo: b.get_colo(),
            }).collect(),
            sticky_ips: sticky_ips.iter().map(|ip| ip.to_string()).collect(),
        }
    } else {
        StatusInfo {
            running: false,
            uptime_secs: 0,
            next_health_check: 0,
            health_check_interval,
            primary_count: 0,
            primary_target: 0,
            backup_count: 0,
            backup_target: 0,
            primary_ips: vec![],
            backup_ips: vec![],
            sticky_ips: vec![],
        }
    }
}

#[frb]
pub fn get_config() -> ConfigInfo {
    let service = get_service();
    let config = service.get_config();
    ConfigInfo {
        ip_file: config.ip_file,
        http: config.http,
        delay_limit: config.delay_limit,
        tlr: config.tlr,
        ips: config.ips as i32,
        threads: config.threads as i32,
        tls_port: config.tls_port as i32,
        http_port: config.http_port as i32,
        max_sticky_slots: config.max_sticky_slots as i32,
        listen_addr: config.listen_addr.to_string(),
    }
}

#[frb]
pub fn get_logs() -> Vec<LogItem> {
    let buffer = cfnat::log::get_log_buffer();
    buffer.get_all().iter().map(|log| LogItem {
        timestamp: log.timestamp.clone(),
        level: log.level.clone(),
        message: log.message.clone(),
    }).collect()
}

#[frb]
pub fn clear_logs() {
    let buffer = cfnat::log::get_log_buffer();
    buffer.clear();
}