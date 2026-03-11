use std::collections::HashSet;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

const DELAY_SAMPLE_WINDOW: usize = 20;
const MIN_SAMPLES_FOR_WEIGHT: usize = 20;
const PING_TIMES: u8 = 4;

fn lcg_random(seed: u64) -> u64 {
    const LCG_A: u64 = 6364136223846793005;
    const LCG_C: u64 = 1442695040888963407;
    seed.wrapping_mul(LCG_A).wrapping_add(LCG_C)
}

pub(crate) struct Backend {
    pub(crate) addr: SocketAddr,
    connections: AtomicUsize,
    delay_samples: std::sync::Mutex<VecDeque<f32>>,
    loss_samples: std::sync::Mutex<VecDeque<bool>>,
    removed: AtomicBool,
}

impl Backend {
    pub(crate) fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            connections: AtomicUsize::new(0),
            delay_samples: std::sync::Mutex::new(VecDeque::with_capacity(DELAY_SAMPLE_WINDOW)),
            loss_samples: std::sync::Mutex::new(VecDeque::with_capacity(DELAY_SAMPLE_WINDOW)),
            removed: AtomicBool::new(false),
        }
    }

    pub(crate) fn record_delay(&self, delay_ms: f32) {
        let mut samples = self.delay_samples.lock().unwrap();
        samples.push_back(delay_ms);
        if samples.len() > DELAY_SAMPLE_WINDOW {
            samples.pop_front();
        }
    }

    pub(crate) fn record_loss(&self, is_loss: bool) {
        let mut samples = self.loss_samples.lock().unwrap();
        samples.push_back(is_loss);
        if samples.len() > DELAY_SAMPLE_WINDOW {
            samples.pop_front();
        }
    }

    pub(crate) fn get_avg_delay(&self) -> f32 {
        let samples = self.delay_samples.lock().unwrap();
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<f32>() / samples.len() as f32
        }
    }

    pub(crate) fn get_loss_rate(&self) -> f32 {
        let samples = self.loss_samples.lock().unwrap();
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().filter(|&&l| l).count() as f32 / samples.len() as f32
        }
    }

    pub(crate) fn get_sample_count(&self) -> usize {
        let samples = self.delay_samples.lock().unwrap();
        samples.len()
    }

    pub(crate) fn is_removed(&self) -> bool {
        self.removed.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_removed(&self) {
        self.removed.store(true, Ordering::Relaxed);
    }

    fn get_weight(&self, delay_threshold: f32) -> f32 {
        let sample_count = self.get_sample_count();
        if sample_count < MIN_SAMPLES_FOR_WEIGHT {
            return 1.0;
        }

        let avg_delay = self.get_avg_delay();
        let loss_rate = self.get_loss_rate();

        if avg_delay <= 0.0 {
            return 1.0;
        }

        let base_weight = 1.0;
        let loss_factor = 1.0 - loss_rate;
        let delay_factor = delay_threshold / avg_delay;

        base_weight * loss_factor * delay_factor
    }
}

pub(crate) struct LoadBalancer {
    primary: std::sync::RwLock<Vec<Arc<Backend>>>,
    backup: std::sync::RwLock<Vec<Arc<Backend>>>,
    ip_set: std::sync::RwLock<HashSet<std::net::IpAddr>>,
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
}

impl LoadBalancer {
    pub(crate) fn new(primary_target: usize) -> Self {
        let backup_target = (primary_target as f32 * 0.5).ceil() as usize;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        Self {
            primary: std::sync::RwLock::new(Vec::new()),
            backup: std::sync::RwLock::new(Vec::new()),
            ip_set: std::sync::RwLock::new(HashSet::new()),
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

    pub(crate) fn contains(&self, ip: std::net::IpAddr) -> bool {
        self.ip_set.read().unwrap().contains(&ip)
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

    fn select_round_robin(&self, candidates: &[Arc<Backend>]) -> Arc<Backend> {
        let idx = self.current.fetch_add(1, Ordering::Relaxed);
        candidates[idx % candidates.len()].clone()
    }

    fn select_weighted(&self, candidates: &[Arc<Backend>]) -> Arc<Backend> {
        let delay_threshold = if self.delay_limit > 0.0 {
            self.delay_limit
        } else {
            self.delay_threshold
        };

        let weights: Vec<f32> = candidates
            .iter()
            .map(|b| b.get_weight(delay_threshold))
            .collect();

        let total_weight: f32 = weights.iter().sum();
        
        if total_weight <= 0.0 {
            return self.select_round_robin(candidates);
        }

        let seed = self.random_seed.fetch_add(1, Ordering::Relaxed);
        let r = (lcg_random(seed) as f64 / u64::MAX as f64) as f32 * total_weight;

        let mut acc = 0.0f32;
        for (i, &w) in weights.iter().enumerate() {
            acc += w;
            if acc >= r {
                return candidates[i].clone();
            }
        }

        candidates.last().unwrap().clone()
    }

    pub(crate) fn select(&self) -> Option<Arc<Backend>> {
        let primary = self.primary.read().unwrap();
        
        let candidates: Vec<Arc<Backend>> = primary
            .iter()
            .filter(|b| !b.is_removed())
            .cloned()
            .collect();

        if candidates.is_empty() {
            drop(primary);
            let backup = self.backup.read().unwrap();
            let candidates: Vec<Arc<Backend>> = backup
                .iter()
                .filter(|b| !b.is_removed())
                .cloned()
                .collect();
            drop(backup);
            
            if candidates.is_empty() {
                return None;
            }
            
            let selected = self.select_round_robin(&candidates);
            selected.connections.fetch_add(1, Ordering::Relaxed);
            return Some(selected);
        }

        let seed = self.random_seed.fetch_add(1, Ordering::Relaxed);
        let r = lcg_random(seed) % 100;
        
        let selected = if r < 20 {
            self.select_round_robin(&candidates)
        } else {
            self.select_weighted(&candidates)
        };

        selected.connections.fetch_add(1, Ordering::Relaxed);
        Some(selected)
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
        
        if sample_count < MIN_SAMPLES_FOR_WEIGHT {
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
        
        let mut primary = self.primary.write().unwrap();
        primary.retain(|b| b.addr.ip() != ip);
        
        let mut backup = self.backup.write().unwrap();
        backup.retain(|b| b.addr.ip() != ip);
        
        self.ip_set.write().unwrap().remove(&ip);
        
        println!("[移除] IP {} 已从所有队列移除 ← 剔除或健康检查", ip);
        self.notify_resume();
    }

    pub(crate) fn refill_from_backup(&self) {
        let primary_len = {
            let primary = self.primary.read().unwrap();
            primary.len()
        };

        if primary_len < self.primary_target {
            let mut backup = self.backup.write().unwrap();
            let mut primary = self.primary.write().unwrap();
            
            while primary.len() < self.primary_target && !backup.is_empty() {
                let backend = backup.remove(0);
                let addr = backend.addr;
                primary.push(backend);
                println!("[补位] {} 从备选提升到主队列 ← 队列需要填充", addr);
            }
        }

        // 备选队列可能有空缺，通知测速协程填充
        self.notify_resume();
    }

    pub(crate) fn get_backup_count(&self) -> usize {
        self.backup.read().unwrap().len()
    }

    pub(crate) fn get_primary_count(&self) -> usize {
        self.primary.read().unwrap().len()
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
        self.primary.write().unwrap().push(backend);
        self.ip_set.write().unwrap().insert(ip);
    }

    pub(crate) fn add_to_backup(&self, addr: SocketAddr) {
        let ip = addr.ip();
        if self.contains(ip) {
            return;
        }
        
        let backend = Arc::new(Backend::new(addr));
        self.backup.write().unwrap().push(backend);
        self.ip_set.write().unwrap().insert(ip);
    }

    pub(crate) fn get_backup_backends(&self) -> Vec<Arc<Backend>> {
        self.backup.read().unwrap().clone()
    }

    pub(crate) fn start_health_check(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            let (_, host, scheme, path) = match crate::hyper::parse_url(&self.health_check_url) {
                Some(r) => r,
                None => return,
            };

            let client = match crate::hyper::build_hyper_client(self.timeout_ms, host.clone()) {
                Some(c) => c,
                None => return,
            };

            loop {
                interval.tick().await;

                let backends = self.get_backup_backends();
                
                if backends.is_empty() {
                    continue;
                }

                println!("[健康检查] 开始检查 {} 个备选 IP ← 每分钟主动测速", backends.len());

                for backend in backends {
                    if backend.is_removed() {
                        continue;
                    }

                    let result = crate::httping::ping_single_ip(
                        backend.addr.ip(),
                        self.tls_port,
                        self.http_port,
                        &client,
                        &host,
                        &scheme,
                        &path,
                        self.timeout_ms,
                    ).await;

                    match result {
                        Some((delay, success_count)) => {
                            backend.record_delay(delay);
                            
                            let is_loss = success_count < PING_TIMES as u8;
                            backend.record_loss(is_loss);
                            
                            let avg_delay = backend.get_avg_delay();
                            
                            if avg_delay > self.delay_threshold {
                                println!("[健康检查] {} 平均延迟 {:.1}ms 超阈值，移除 ← 每分钟主动测速", backend.addr, avg_delay);
                                self.remove_backend(backend.clone());
                            } else {
                                println!("[健康检查] {} 延迟 {:.1}ms ({}/{}) 正常 ← 每分钟主动测速", backend.addr, delay, success_count, PING_TIMES);
                            }
                        }
                        None => {
                            backend.record_loss(true);
                            
                            let loss_rate = backend.get_loss_rate();
                            if loss_rate > self.loss_threshold {
                                println!("[健康检查] {} 丢包率 {:.1}% 超阈值，移除 ← 每分钟主动测速", backend.addr, loss_rate * 100.0);
                                self.remove_backend(backend.clone());
                            } else {
                                println!("[健康检查] {} 测速失败 ← 每分钟主动测速", backend.addr);
                            }
                        }
                    }
                }
            }
        });
    }
}
