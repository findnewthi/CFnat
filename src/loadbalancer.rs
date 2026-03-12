use std::collections::HashSet;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use parking_lot::Mutex;
use parking_lot::RwLock;

const SAMPLE_WINDOW: usize = 20;
const PING_TIMES: u8 = 4;
const HEALTH_CHECK_CONCURRENCY: usize = 4;
const DELAY_SMOOTHING: f32 = 5.0;
const MAX_PROBE_ATTEMPTS: usize = 3;
const SESSION_CACHE_SIZE: usize = 128;

fn lcg_random(seed: u64) -> u64 {
    const LCG_A: u64 = 6364136223846793005;
    const LCG_C: u64 = 1442695040888963407;
    seed.wrapping_mul(LCG_A).wrapping_add(LCG_C)
}

struct SessionCache {
    cache: Mutex<[(std::net::IpAddr, SocketAddr, u128); SESSION_CACHE_SIZE]>,
    size: AtomicUsize,
    cursor: AtomicUsize,
    ttl: Duration,
}

impl SessionCache {
    fn new() -> Self {
        Self {
            cache: Mutex::new([(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 
                     SocketAddr::V4(std::net::SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, 0)), 
                     0); SESSION_CACHE_SIZE]),
            size: AtomicUsize::new(0),
            cursor: AtomicUsize::new(0),
            ttl: Duration::from_secs(30),
        }
    }
    
    fn get(&self, client_ip: &std::net::IpAddr) -> Option<SocketAddr> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let size = self.size.load(Ordering::Relaxed);
        
        let cache = self.cache.lock();
        for i in 0..size {
            let (ip, addr, expiry) = cache[i];
            if ip == *client_ip && now < expiry {
                return Some(addr);
            }
        }
        None
    }
    
    fn put(&self, client_ip: std::net::IpAddr, backend_addr: SocketAddr) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let expiry = now + self.ttl.as_nanos();
        let size = self.size.load(Ordering::Relaxed);
        
        let mut cache = self.cache.lock();
        for i in 0..size {
            let (ip, _, _) = cache[i];
            if ip == client_ip {
                cache[i] = (client_ip, backend_addr, expiry);
                return;
            }
        }
        
        if size < SESSION_CACHE_SIZE {
            cache[size] = (client_ip, backend_addr, expiry);
            self.size.store(size + 1, Ordering::Relaxed);
            return;
        }
        
        let idx = self.cursor.fetch_add(1, Ordering::Relaxed) % SESSION_CACHE_SIZE;
        cache[idx] = (client_ip, backend_addr, expiry);
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

    pub(crate) fn get_connections(&self) -> usize {
        self.connections.load(Ordering::Relaxed)
    }

    pub(crate) fn is_removed(&self) -> bool {
        self.removed.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_removed(&self) {
        self.removed.store(true, Ordering::Relaxed);
    }

    fn get_weight(&self, delay_threshold: f32) -> f32 {
        let sample_count = self.get_sample_count();
        if sample_count < SAMPLE_WINDOW {
            return 1.0;
        }

        let avg_delay = self.get_avg_delay();
        let loss_rate = self.get_loss_rate();

        if avg_delay <= 0.0 {
            return 1.0;
        }

        let base_weight = 1.0;
        let loss_factor = 1.0 - loss_rate;
        let delay_factor = delay_threshold / (avg_delay + DELAY_SMOOTHING);

        let connections = self.get_connections() as f32;
        let load_factor = 1.0 / (1.0 + connections / 10.0);

        let raw_weight = base_weight * loss_factor * delay_factor * load_factor;
        
        let max_weight = delay_threshold / (DELAY_SMOOTHING + delay_threshold * 0.3);
        raw_weight.min(max_weight)
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
    current: AtomicUsize,
    random_seed: AtomicU64,
    primary_target: usize,
    backup_target: usize,
    delay_limit: f32,
    health_check_url: String,
    tls_port: u16,
    http_port: u16,
    timeout_ms: u64,
    notify_tx: Option<tokio::sync::watch::Sender<bool>>,
    delay_threshold: f32,
    loss_threshold: f32,
    client: Option<Arc<crate::hyper::MyHyperClient>>,
    session_cache: Mutex<SessionCache>,
    colo_filter: Option<Arc<Vec<String>>>,
}

impl LoadBalancer {
    pub(crate) fn new(primary_target: usize) -> Self {
        let backup_target = (primary_target as f32 * 0.5).ceil() as usize;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let session_cache = Mutex::new(SessionCache::new());
        Self {
            primary: RwLock::new(Vec::new()),
            backup: RwLock::new(Vec::new()),
            ip_set: RwLock::new(HashSet::new()),
            current: AtomicUsize::new(0),
            random_seed: AtomicU64::new(seed),
            primary_target,
            backup_target,
            delay_limit: 0.0,
            health_check_url: String::new(),
            tls_port: 443,
            http_port: 80,
            timeout_ms: 2000,
            notify_tx: None,
            delay_threshold: 0.0,
            loss_threshold: 0.0,
            client: None,
            session_cache,
            colo_filter: None,
        }
    }

    pub(crate) fn with_delay_limit(mut self, delay_limit: f32) -> Self {
        self.delay_limit = delay_limit;
        self
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

    fn select_round_robin(&self, backends: &[Arc<Backend>], start_idx: usize) -> Option<Arc<Backend>> {
        let len = backends.len();
        for i in 0..len {
            let idx = (start_idx + i) % len;
            let backend = &backends[idx];
            if !backend.is_removed() {
                return Some(backend.clone());
            }
        }
        None
    }

    fn select_power_of_two(&self, backends: &[Arc<Backend>], delay_threshold: f32) -> Option<Arc<Backend>> {
        let len = backends.len();
        if len == 0 {
            return None;
        }

        let seed1 = self.random_seed.fetch_add(1, Ordering::Relaxed);
        let seed2 = self.random_seed.fetch_add(1, Ordering::Relaxed);
        
        let mut idx1 = (lcg_random(seed1) as usize) % len;
        let mut idx2 = (lcg_random(seed2) as usize) % len;
        
        let mut b1 = &backends[idx1];
        let mut b2 = &backends[idx2];
        
        let mut attempts = 0;
        while b1.is_removed() && attempts < MAX_PROBE_ATTEMPTS {
            idx1 = (idx1 + 1) % len;
            b1 = &backends[idx1];
            attempts += 1;
        }
        
        if b1.is_removed() {
            return self.select_round_robin(backends, idx1);
        }
        
        attempts = 0;
        while b2.is_removed() && attempts < MAX_PROBE_ATTEMPTS {
            idx2 = (idx2 + 1) % len;
            b2 = &backends[idx2];
            attempts += 1;
        }
        
        if b2.is_removed() {
            return Some(b1.clone());
        }

        let w1 = b1.get_weight(delay_threshold);
        let w2 = b2.get_weight(delay_threshold);

        if w1 >= w2 {
            Some(b1.clone())
        } else {
            Some(b2.clone())
        }
    }

    pub(crate) fn select(&self, client_ip: std::net::IpAddr) -> Option<Arc<Backend>> {
        {
            let cache = self.session_cache.lock();
            if let Some(backend_addr) = cache.get(&client_ip) {
                let ip_set = self.ip_set.read();
                if ip_set.contains(&backend_addr.ip()) {
                    drop(ip_set);
                    let primary = self.primary.read();
                    let backend = primary.iter()
                        .find(|b| b.addr == backend_addr)
                        .cloned();
                    drop(primary);
                    
                    if let Some(backend) = backend {
                        backend.connections.fetch_add(1, Ordering::Relaxed);
                        return Some(backend);
                    }
                }
            }
        }
        
        let primary = self.primary.read();
        
        if primary.is_empty() {
            drop(primary);
            let backup = self.backup.read();
            if backup.is_empty() {
                return None;
            }
            let idx = self.current.fetch_add(1, Ordering::Relaxed);
            if let Some(selected) = self.select_round_robin(&backup, idx) {
                selected.connections.fetch_add(1, Ordering::Relaxed);
                let cache = self.session_cache.lock();
                cache.put(client_ip, selected.addr);
                return Some(selected);
            }
            return None;
        }

        let idx = self.current.fetch_add(1, Ordering::Relaxed);
        
        let delay_threshold = if self.delay_limit > 0.0 {
            self.delay_limit
        } else {
            self.delay_threshold
        };

        let seed = self.random_seed.fetch_add(1, Ordering::Relaxed);
        let r = lcg_random(seed) % 100;
        
        let selected = if r < 20 {
            self.select_round_robin(&primary, idx)
        } else {
            self.select_power_of_two(&primary, delay_threshold)
        };

        drop(primary);

        if let Some(backend) = selected {
            backend.connections.fetch_add(1, Ordering::Relaxed);
            let cache = self.session_cache.lock();
            cache.put(client_ip, backend.addr);
            return Some(backend);
        }

        None
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