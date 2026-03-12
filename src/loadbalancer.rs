use std::collections::HashSet;
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use parking_lot::RwLock;

const SAMPLE_WINDOW: usize = 20;
const PING_TIMES: u8 = 4;
const HEALTH_CHECK_CONCURRENCY: usize = 4;

fn fast_hash_ip_port(ip: IpAddr, port: u16) -> usize {
    let mut h = match ip {
        IpAddr::V4(v4) => v4.to_bits() as u64,
        IpAddr::V6(v6) => {
            let bits = v6.to_bits();
            ((bits >> 64) ^ bits) as u64
        }
    };
    h = h.wrapping_add(port as u64);
    h = (h ^ (h >> 33)).wrapping_mul(0xff51afd7ed558ccd);
    h as usize
}

fn is_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

pub(crate) struct Backend {
    pub(crate) addr: SocketAddr,
    connections: AtomicUsize,
    delay_samples: Mutex<VecDeque<f32>>,
    loss_samples: Mutex<VecDeque<bool>>,
    removed: AtomicBool,
}

impl Backend {
    pub(crate) fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            connections: AtomicUsize::new(0),
            delay_samples: Mutex::new(VecDeque::with_capacity(SAMPLE_WINDOW)),
            loss_samples: Mutex::new(VecDeque::with_capacity(SAMPLE_WINDOW)),
            removed: AtomicBool::new(false),
        }
    }

    pub(crate) fn record_delay(&self, delay_ms: f32) {
        let mut samples = self.delay_samples.lock();
        samples.push_back(delay_ms);
        if samples.len() > SAMPLE_WINDOW {
            samples.pop_front();
        }
    }

    pub(crate) fn record_loss(&self, is_loss: bool) {
        let mut samples = self.loss_samples.lock();
        samples.push_back(is_loss);
        if samples.len() > SAMPLE_WINDOW {
            samples.pop_front();
        }
    }

    pub(crate) fn get_avg_delay(&self) -> f32 {
        let samples = self.delay_samples.lock();
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<f32>() / samples.len() as f32
        }
    }

    pub(crate) fn get_loss_rate(&self) -> f32 {
        let samples = self.loss_samples.lock();
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().filter(|&&l| l).count() as f32 / samples.len() as f32
        }
    }

    pub(crate) fn get_sample_count(&self) -> usize {
        self.loss_samples.lock().len()
    }

    pub(crate) fn is_removed(&self) -> bool {
        self.removed.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_removed(&self) {
        self.removed.store(true, Ordering::Relaxed);
    }
}

struct HealthCheckConfig {
    client: Arc<crate::hyper::MyHyperClient>,
    host: Arc<str>,
    scheme: Arc<str>,
    path: Arc<str>,
    timeout_ms: u64,
    tls_port: u16,
    http_port: u16,
    delay_threshold: f32,
    loss_threshold: f32,
    colo_filter: Option<Arc<Vec<String>>>,
}

pub(crate) struct LoadBalancer {
    primary: RwLock<Vec<Arc<Backend>>>,
    backup: RwLock<Vec<Arc<Backend>>>,
    ip_set: RwLock<HashSet<std::net::IpAddr>>,
    primary_target: usize,
    backup_target: usize,
    health_check_url: String,
    tls_port: u16,
    http_port: u16,
    timeout_ms: u64,
    notify_tx: Option<tokio::sync::watch::Sender<bool>>,
    delay_threshold: f32,
    loss_threshold: f32,
    client: Option<Arc<crate::hyper::MyHyperClient>>,
    colo_filter: Option<Arc<Vec<String>>>,
}

impl LoadBalancer {
    pub(crate) fn new(primary_target: usize) -> Self {
        let backup_target = (primary_target as f32 * 0.5).ceil() as usize;
        Self {
            primary: RwLock::new(Vec::new()),
            backup: RwLock::new(Vec::new()),
            ip_set: RwLock::new(HashSet::new()),
            primary_target,
            backup_target,
            health_check_url: String::new(),
            tls_port: 443,
            http_port: 80,
            timeout_ms: 2000,
            notify_tx: None,
            delay_threshold: 0.0,
            loss_threshold: 0.0,
            client: None,
            colo_filter: None,
        }
    }

    pub(crate) fn with_delay_threshold(mut self, delay_threshold: f32) -> Self {
        self.delay_threshold = delay_threshold;
        self
    }

    pub(crate) fn with_loss_threshold(mut self, loss_threshold: f32) -> Self {
        self.loss_threshold = loss_threshold;
        self
    }

    pub(crate) fn with_colo_filter(mut self, colo_filter: Option<Vec<String>>) -> Self {
        self.colo_filter = colo_filter.map(|v| Arc::new(v));
        self
    }

    pub(crate) fn get_delay_threshold(&self) -> f32 {
        self.delay_threshold
    }

    pub(crate) fn with_health_check_url(mut self, url: String) -> Self {
        self.health_check_url = url;
        self
    }

    pub(crate) fn with_ports(mut self, tls_port: u16, http_port: u16) -> Self {
        self.tls_port = tls_port;
        self.http_port = http_port;
        self
    }

    pub(crate) fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub(crate) fn with_notify(mut self, tx: tokio::sync::watch::Sender<bool>) -> Self {
        self.notify_tx = Some(tx);
        self
    }

    pub(crate) fn with_client(mut self, client: Arc<crate::hyper::MyHyperClient>) -> Self {
        self.client = Some(client);
        self
    }

    pub(crate) fn contains(&self, ip: std::net::IpAddr) -> bool {
        self.ip_set.read().contains(&ip)
    }

    pub(crate) fn primary_full(&self) -> bool {
        self.get_primary_count() >= self.primary_target
    }

    pub(crate) fn backup_full(&self) -> bool {
        self.get_backup_count() >= self.backup_target
    }

    pub(crate) fn should_pause(&self) -> bool {
        self.primary_full() && self.backup_full()
    }

    pub(crate) fn notify_resume(&self) {
        if let Some(tx) = &self.notify_tx {
            let _ = tx.send(true);
        }
    }

    pub(crate) fn select(&self, client_ip: IpAddr, source_port: u16) -> Option<Arc<Backend>> {
        let primary = self.primary.read();
        
        if !primary.is_empty() {
            return self.select_backend(client_ip, source_port, &primary);
        }
        
        drop(primary);
        let backup = self.backup.read();
        
        if backup.is_empty() {
            return None;
        }
        
        self.select_backend(client_ip, source_port, &backup)
    }

    fn select_backend(&self, client_ip: IpAddr, source_port: u16, pool: &[Arc<Backend>]) -> Option<Arc<Backend>> {
        let len = pool.len();
        if len == 0 {
            return None;
        }

        let base_hash = if is_local_ip(client_ip) {
            source_port as usize
        } else {
            fast_hash_ip_port(client_ip, source_port)
        };

        let idx1 = base_hash % len;
        let idx2 = (base_hash + 1) % len;

        let b1 = &pool[idx1];
        let b2 = if len > 1 { Some(&pool[idx2]) } else { None };

        let selected = match b2 {
            Some(inner_b2) => {
                let c1 = b1.connections.load(Ordering::Relaxed);
                let c2 = inner_b2.connections.load(Ordering::Relaxed);

                match (b1.is_removed(), inner_b2.is_removed()) {
                    (false, false) => {
                        if c1 <= c2 { b1 } else { inner_b2 }
                    }
                    (false, true) => b1,
                    (true, false) => inner_b2,
                    (true, true) => return None,
                }
            }
            None => {
                if b1.is_removed() {
                    return None;
                }
                b1
            }
        };

        selected.connections.fetch_add(1, Ordering::Relaxed);
        Some(selected.clone())
    }

    fn cleanup_removed(&self) {
        let mut primary = self.primary.write();
        let before = primary.len();
        primary.retain(|b| !b.is_removed());
        drop(primary);

        let mut backup = self.backup.write();
        backup.retain(|b| !b.is_removed());
        drop(backup);

        let removed_count = before - self.primary.read().len();
        if removed_count > 0 {
            println!("[清理] 移除 {} 个失效节点 ← 后台清理", removed_count);
        }
    }

    pub(crate) fn release(&self, backend: &Backend) {
        backend.connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn record_delay(&self, backend: &Backend, delay_ms: f32) {
        backend.record_delay(delay_ms);
    }

    pub(crate) fn record_loss(&self, backend: &Backend, is_loss: bool) {
        backend.record_loss(is_loss);
    }

    pub(crate) fn check_and_evict(&self, backend: &Backend) -> bool {
        let sample_count = backend.get_sample_count();
        
        if sample_count < SAMPLE_WINDOW {
            return false;
        }
        
        let avg_delay = backend.get_avg_delay();
        let loss_rate = backend.get_loss_rate();

        if avg_delay > self.delay_threshold || loss_rate > self.loss_threshold {
            backend.mark_removed();
            true
        } else {
            false
        }
    }

    pub(crate) fn remove_backend(&self, backend: Arc<Backend>) {
        let ip = backend.addr.ip();
        
        self.primary.write().retain(|b| b.addr.ip() != ip);
        self.backup.write().retain(|b| b.addr.ip() != ip);
        self.ip_set.write().remove(&ip);
        
        println!("[移除] IP {} 已从所有队列移除 ← 剔除或健康检查", ip);
        self.notify_resume();
    }

    pub(crate) fn refill_from_backup(&self) {
        let primary_len = {
            self.primary.read().len()
        };

        if primary_len < self.primary_target {
            let mut backup = self.backup.write();
            let mut primary = self.primary.write();
            
            while primary.len() < self.primary_target && !backup.is_empty() {
                let backend = backup.remove(0);
                let addr = backend.addr;
                primary.push(backend);
                println!("[补位] {} 从备选提升到主队列 ← 队列需要填充", addr);
            }
        }

        self.notify_resume();
    }

    pub(crate) fn get_backup_count(&self) -> usize {
        self.backup.read().len()
    }

    pub(crate) fn get_primary_count(&self) -> usize {
        self.primary.read().len()
    }

    pub(crate) fn get_primary_target(&self) -> usize {
        self.primary_target
    }

    pub(crate) fn get_backup_target(&self) -> usize {
        self.backup_target
    }

    pub(crate) fn add_to_primary(&self, addr: SocketAddr) {
        let ip = addr.ip();
        if self.contains(ip) {
            return;
        }
        
        let backend = Arc::new(Backend::new(addr));
        self.primary.write().push(backend);
        self.ip_set.write().insert(ip);
    }

    pub(crate) fn add_to_backup(&self, addr: SocketAddr) {
        let ip = addr.ip();
        if self.contains(ip) {
            return;
        }
        
        let backend = Arc::new(Backend::new(addr));
        self.backup.write().push(backend);
        self.ip_set.write().insert(ip);
    }

    pub(crate) fn get_backup_backends(&self) -> Vec<Arc<Backend>> {
        self.backup.read().clone()
    }

    pub(crate) fn start_health_check(self: Arc<Self>) {
        let client = match &self.client {
            Some(c) => c.clone(),
            None => return,
        };

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            let (_, host, scheme, path) = match crate::hyper::parse_url(&self.health_check_url) {
                Some(r) => r,
                None => return,
            };
            
            let config = Arc::new(HealthCheckConfig {
                client,
                host: Arc::from(host.as_str()),
                scheme: Arc::from(scheme),
                path: Arc::from(path.as_str()),
                timeout_ms: self.timeout_ms,
                tls_port: self.tls_port,
                http_port: self.http_port,
                delay_threshold: self.delay_threshold,
                loss_threshold: self.loss_threshold,
                colo_filter: self.colo_filter.clone(),
            });

            loop {
                interval.tick().await;

                let backends = self.get_backup_backends();
                
                if backends.is_empty() {
                    continue;
                }

                println!("[健康检查] 开始并发检查 {} 个备选 IP ← 每分钟主动测速", backends.len());

                let mut join_set = tokio::task::JoinSet::new();

                for backend in backends {
                    if backend.is_removed() {
                        continue;
                    }

                    let config = config.clone();
                    let lb = self.clone();

                    join_set.spawn(async move {
                        let result = crate::httping::http_ping_multi(
                            backend.addr.ip(),
                            config.tls_port,
                            config.http_port,
                            config.client.clone(),
                            config.host.clone(),
                            config.scheme.clone(),
                            config.path.clone(),
                            config.timeout_ms,
                            config.colo_filter.clone(),
                        ).await;

                        (backend, result, config.delay_threshold, config.loss_threshold, lb)
                    });

                    if join_set.len() >= HEALTH_CHECK_CONCURRENCY {
                        while join_set.len() >= HEALTH_CHECK_CONCURRENCY / 2 {
                            if let Some(res) = join_set.join_next().await {
                                if let Ok((backend, result, delay_threshold, loss_threshold, lb)) = res {
                                    Self::handle_health_check_result(backend, result, delay_threshold, loss_threshold, lb);
                                }
                            }
                        }
                    }
                }

                while let Some(res) = join_set.join_next().await {
                    if let Ok((backend, result, delay_threshold, loss_threshold, lb)) = res {
                        Self::handle_health_check_result(backend, result, delay_threshold, loss_threshold, lb);
                    }
                }

                self.cleanup_removed();
            }
        });
    }

    fn handle_health_check_result(
        backend: Arc<Backend>,
        result: Option<(std::net::SocketAddr, f32, Option<String>, u8)>,
        delay_threshold: f32,
        loss_threshold: f32,
        lb: Arc<LoadBalancer>,
    ) {
        match result {
            Some((_addr, delay, colo, success_count)) => {
                backend.record_delay(delay);
                
                let is_loss = success_count < PING_TIMES;
                backend.record_loss(is_loss);
                
                let sample_count = backend.get_sample_count();
                let colo_str = colo.as_deref().unwrap_or("???");
                
                if sample_count < SAMPLE_WINDOW {
                    println!("[健康检查] {} 延迟 {:.1}ms (成功{}/{}) [{}] 收集中 {}/{} ← 每分钟主动测速", 
                        backend.addr, delay, success_count, PING_TIMES, colo_str, sample_count, SAMPLE_WINDOW);
                    return;
                }
                
                let avg_delay = backend.get_avg_delay();
                let loss_rate = backend.get_loss_rate();
                
                if avg_delay > delay_threshold {
                    println!("[剔除] {} 延迟 {:.1}ms/阈值 {:.1}ms 丢包率 {:.1}% [{}] ← 每分钟主动测速", 
                        backend.addr, avg_delay, delay_threshold, loss_rate * 100.0, colo_str);
                    lb.remove_backend(backend);
                } else {
                    println!("[健康检查] {} 延迟 {:.1}ms (成功{}/{}) [{}] 正常 ← 每分钟主动测速", 
                        backend.addr, delay, success_count, PING_TIMES, colo_str);
                }
            }
            None => {
                backend.record_loss(true);
                
                let sample_count = backend.get_sample_count();
                if sample_count < SAMPLE_WINDOW {
                    println!("[健康检查] {} 测速失败 (成功0/{}) 收集中 {}/{} ← 每分钟主动测速", 
                        backend.addr, PING_TIMES, sample_count, SAMPLE_WINDOW);
                    return;
                }
                
                let loss_rate = backend.get_loss_rate();
                let avg_delay = backend.get_avg_delay();
                if loss_rate > loss_threshold {
                    println!("[剔除] {} 丢包率 {:.1}%/阈值 {:.1}% 延迟 {:.1}ms ← 每分钟主动测速", 
                        backend.addr, loss_rate * 100.0, loss_threshold * 100.0, avg_delay);
                    lb.remove_backend(backend);
                } else {
                    println!("[健康检查] {} 测速失败 (成功0/{}) ← 每分钟主动测速", backend.addr, PING_TIMES);
                }
            }
        }
    }
}