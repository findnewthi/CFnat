use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::RwLock;
use tokio_util::sync::CancellationToken;

use crate::core::{IpPool, LoadBalancer, HttpingConfig, build_hyper_client, parse_url, run_continuous_httping, run_forward};

pub struct ServiceState {
    pub running: AtomicBool,
    pub ip_pool: RwLock<Option<Arc<IpPool>>>,
    pub loadbalancer: RwLock<Option<Arc<LoadBalancer>>>,
    pub config: RwLock<ServiceConfig>,
    pub cancel_token: RwLock<Option<CancellationToken>>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceConfig {
    pub ip_file: String,
    pub http: String,
    pub delay_limit: u64,
    pub tlr: f64,
    pub ips: usize,
    pub threads: usize,
    pub tls_port: u16,
    pub http_port: u16,
    pub colo: Option<Vec<String>>,
    pub listen_addr: SocketAddr,
    pub api_addr: SocketAddr,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            ip_file: "ip.txt".to_string(),
            http: "http://cp.cloudflare.com/cdn-cgi/trace".to_string(),
            delay_limit: 500,
            tlr: 0.1,
            ips: 10,
            threads: 16,
            tls_port: 443,
            http_port: 80,
            colo: None,
            listen_addr: "127.6.6.6:6".parse().unwrap(),
            api_addr: "127.0.0.1:0".parse().unwrap(),
        }
    }
}

impl Default for ServiceState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceState {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            ip_pool: RwLock::new(None),
            loadbalancer: RwLock::new(None),
            config: RwLock::new(ServiceConfig::default()),
            cancel_token: RwLock::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn get_config(&self) -> ServiceConfig {
        self.config.read().clone()
    }

    pub fn update_config(&self, new_config: ServiceConfig) {
        *self.config.write() = new_config;
    }

    pub fn start(&self) -> Result<(), String> {
        if self.is_running() {
            return Err("服务已在运行".to_string());
        }

        let config = self.get_config();
        
        let ip_pool = Arc::new(IpPool::from_file(&config.ip_file));
        if ip_pool.total_count() == 0 {
            return Err("未找到有效的 IP".to_string());
        }

        crate::core::init_global_limiter(config.threads);

        let (_, host, _, _) = parse_url(&config.http)
            .ok_or("URL 解析失败")?;

        let client = build_hyper_client(config.delay_limit, host.clone())
            .ok_or("创建 HTTP 客户端失败")?;

        let client = Arc::new(client);
        let (notify_tx, notify_rx) = tokio::sync::watch::channel(false);
        let colo_filter = config.colo.clone();

        let cancel_token = CancellationToken::new();
        let cancel_token_clone = cancel_token.clone();
        
        let lb = Arc::new(
            LoadBalancer::new(config.ips)
                .with_delay_threshold(config.delay_limit as f32)
                .with_loss_threshold(config.tlr as f32)
                .with_health_check_url(config.http.clone())
                .with_ports(config.tls_port, config.http_port)
                .with_timeout(1800)
                .with_notify(notify_tx)
                .with_client(client.clone())
                .with_colo_filter(colo_filter.clone()),
        );

        *self.ip_pool.write() = Some(ip_pool.clone());
        *self.loadbalancer.write() = Some(lb.clone());
        *self.cancel_token.write() = Some(cancel_token);
        self.running.store(true, Ordering::Relaxed);

        let ip_pool_clone = ip_pool.clone();
        let lb_clone = lb.clone();
        let http = config.http.clone();
        let tls_port = config.tls_port;
        let http_port = config.http_port;
        let delay_limit = config.delay_limit;

        tokio::spawn(async move {
            run_continuous_httping(
                ip_pool_clone,
                lb_clone.clone(),
                &http,
                HttpingConfig {
                    tls_port,
                    http_port,
                    timeout_ms: 1800,
                    delay_limit,
                    colo_filter: colo_filter.map(Arc::new),
                    client,
                },
                notify_rx,
                cancel_token_clone,
            ).await;
        });

        lb.clone().start_health_check();

        let listen_addr = config.listen_addr;
        let lb_forward = lb.clone();
        let tls_port = config.tls_port;
        let http_port = config.http_port;
        
        tokio::spawn(async move {
            if let Err(e) = run_forward(listen_addr, lb_forward, tls_port, http_port).await {
                eprintln!("转发服务错误: {}", e);
            }
        });

        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        if !self.is_running() {
            return Err("服务未运行".to_string());
        }

        if let Some(lb) = self.loadbalancer.read().clone() {
            lb.stop();
        }

        if let Some(token) = self.cancel_token.read().clone() {
            token.cancel();
        }
        
        self.running.store(false, Ordering::Relaxed);
        *self.ip_pool.write() = None;
        *self.loadbalancer.write() = None;
        *self.cancel_token.write() = None;

        println!("服务已停止");
        
        Ok(())
    }

    pub fn get_loadbalancer(&self) -> Option<Arc<LoadBalancer>> {
        self.loadbalancer.read().clone()
    }
}