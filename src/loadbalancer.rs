use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use parking_lot::RwLock;

const SAMPLE_WINDOW: f32 = 50.0; // EWMA 采样窗口大小
const ALPHA: f32 = 2.0 / (SAMPLE_WINDOW + 1.0); // EWMA 平滑系数
const EVICT_THRESHOLD: usize = 20; // 剔除检查的最小采样数
const MAX_SMOOTH_RATIO: f32 = 5.0; // 最大平滑比率
const MAX_BACKUP_TARGET: usize = 10; // 最大备选节点数
const PING_TIMES: u8 = 4; // 每次健康检查的 ping 次数
const HEALTH_CHECK_CONCURRENCY: usize = 4; // 健康检查并发数
const HEALTH_CHECK_INTERVAL_SECS: u64 = 25; // 健康检查间隔（秒）
const WARMING_DURATION_SECS: u64 = 60; // 预热持续时间（秒）
const STICKY_BASE_SECS: u64 = 10; // 粘滞基础时间（秒）
const STICKY_INCREMENT_SECS: u64 = 5; // 粘滞时间增量（秒）
const MAX_STICKY_SLOTS: usize = 5; // 最大粘滞槽位数
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum BackendState {
    Warming = 0,
    Active = 1,
    Removed = 2,
}

struct StickySlot {
    backend: Arc<Backend>,
    last_switch: Instant,
    last_access: Instant,
    interval: Duration,
}

pub(crate) struct Backend {
    pub(crate) addr: SocketAddr,
    connections: AtomicUsize,
    avg_delay: Mutex<f32>,
    avg_loss: Mutex<f32>,
    sample_count: AtomicUsize,
    state: AtomicU8,
    entered_state_at: Mutex<Instant>,
}

impl Backend {
    pub(crate) fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            connections: AtomicUsize::new(0),
            avg_delay: Mutex::new(-1.0),
            avg_loss: Mutex::new(-1.0),
            sample_count: AtomicUsize::new(0),
            state: AtomicU8::new(BackendState::Warming as u8),
            entered_state_at: Mutex::new(Instant::now()),
        }
    }

    pub(crate) fn new_with_initial(addr: SocketAddr, initial_delay: f32, initial_loss: f32) -> Self {
        Self {
            addr,
            connections: AtomicUsize::new(0),
            avg_delay: Mutex::new(initial_delay),
            avg_loss: Mutex::new(initial_loss),
            sample_count: AtomicUsize::new(0),
            state: AtomicU8::new(BackendState::Warming as u8),
            entered_state_at: Mutex::new(Instant::now()),
        }
    }

    fn update_ewma(current: &mut f32, new_val: f32, is_first: bool) {
        if is_first {
            *current = new_val;
        } else {
            *current = (*current * (1.0 - ALPHA)) + (new_val * ALPHA);
        }
    }

    pub(crate) fn record_delay(&self, delay_ms: f32) {
        let is_first = self.sample_count.load(Ordering::Relaxed) == 0;
        let mut lock = self.avg_delay.lock();
        Self::update_ewma(&mut lock, delay_ms, is_first);
        
        self.sample_count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            Some((count + 1).min(SAMPLE_WINDOW as usize))
        }).ok();
    }

    pub(crate) fn record_loss(&self, is_loss: bool) {
        let is_first = self.sample_count.load(Ordering::Relaxed) == 0;
        let mut lock = self.avg_loss.lock();
        let loss = if is_loss { 1.0 } else { 0.0 };
        Self::update_ewma(&mut lock, loss, is_first);
    }

    pub(crate) fn get_avg_delay(&self) -> f32 {
        self.avg_delay.lock().max(0.0)
    }

    pub(crate) fn get_loss_rate(&self) -> f32 {
        self.avg_loss.lock().max(0.0)
    }

    pub(crate) fn get_sample_count(&self) -> usize {
        self.sample_count.load(Ordering::Relaxed)
    }

    pub(crate) fn is_removed(&self) -> bool {
        self.state.load(Ordering::Relaxed) == BackendState::Removed as u8
    }

    pub(crate) fn is_warming(&self) -> bool {
        self.state.load(Ordering::Relaxed) == BackendState::Warming as u8
    }

    pub(crate) fn is_active(&self) -> bool {
        self.state.load(Ordering::Relaxed) == BackendState::Active as u8
    }

    pub(crate) fn mark_removed(&self) {
        self.state.store(BackendState::Removed as u8, Ordering::Relaxed);
        *self.entered_state_at.lock() = Instant::now();
    }

    fn mark_active(&self) {
        self.state.store(BackendState::Active as u8, Ordering::Relaxed);
        *self.entered_state_at.lock() = Instant::now();
    }

    fn check_warming_expired(&self) -> bool {
        if self.is_warming() {
            let elapsed = self.entered_state_at.lock().elapsed().as_secs();
            elapsed >= WARMING_DURATION_SECS
        } else {
            false
        }
    }

    pub(crate) fn calculate_score(&self, pool_avg_delay: f32, pool_avg_loss: f32) -> f32 {
        let connections = self.connections.load(Ordering::Relaxed) as f32;
        let beta = (self.get_sample_count() as f32 / SAMPLE_WINDOW).min(1.0);
        
        let my_perf = self.get_avg_delay() * (1.0 + self.get_loss_rate() * 2.0);
        let pool_perf = (pool_avg_delay * (1.0 + pool_avg_loss * 2.0)).max(1.0);
        
        let ratio = if pool_perf > 0.0 { my_perf / pool_perf } else { 1.0 };
        let smooth_ratio = (1.0 + beta * (ratio - 1.0)).min(MAX_SMOOTH_RATIO);
        
        (connections + 1.0) * smooth_ratio
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
    primary_index: AtomicUsize,
    backup: RwLock<Vec<Arc<Backend>>>,
    backup_index: AtomicUsize,
    ip_set: RwLock<HashSet<std::net::IpAddr>>,
    primary_target: usize,
    backup_target: usize,
    min_active_target: usize,
    health_check_url: String,
    tls_port: u16,
    http_port: u16,
    timeout_ms: u64,
    notify_tx: Option<tokio::sync::watch::Sender<bool>>,
    delay_threshold: f32,
    loss_threshold: f32,
    client: Option<Arc<crate::hyper::MyHyperClient>>,
    colo_filter: Option<Arc<Vec<String>>>,
    sticky_slots: Mutex<Vec<StickySlot>>,
    last_expand: Mutex<Instant>,
}

impl LoadBalancer {
    pub(crate) fn new(primary_target: usize) -> Self {
        let backup_target = ((primary_target as f32 * 0.5).ceil() as usize).min(MAX_BACKUP_TARGET);
        let min_active_target = (primary_target as f32 / 2.0).ceil() as usize;
        Self {
            primary: RwLock::new(Vec::new()),
            primary_index: AtomicUsize::new(0),
            backup: RwLock::new(Vec::new()),
            backup_index: AtomicUsize::new(0),
            ip_set: RwLock::new(HashSet::new()),
            primary_target,
            backup_target,
            min_active_target,
            health_check_url: String::new(),
            tls_port: 443,
            http_port: 80,
            timeout_ms: 2000,
            notify_tx: None,
            delay_threshold: 0.0,
            loss_threshold: 0.0,
            client: None,
            colo_filter: None,
            sticky_slots: Mutex::new(Vec::new()),
            last_expand: Mutex::new(Instant::now()),
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
        self.colo_filter = colo_filter.map(Arc::new);
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

    pub(crate) fn select(&self) -> Option<Arc<Backend>> {
        let primary = self.primary.read();
        
        if !primary.is_empty() {
            return self.select_backend_p2c(&primary, &self.primary_index);
        }
        
        drop(primary);
        let backup = self.backup.read();
        
        if backup.is_empty() {
            return None;
        }
        
        self.select_backend_p2c(&backup, &self.backup_index)
    }

    fn select_backend_p2c(&self, pool: &[Arc<Backend>], index: &AtomicUsize) -> Option<Arc<Backend>> {
        let now = Instant::now();
        let mut slots = self.sticky_slots.lock();

        slots.retain(|s| now.duration_since(s.last_access) < Duration::from_secs(15));

        let get_active_unused = |used_addrs: &std::collections::HashSet<SocketAddr>| {
            pool.iter()
                .filter(|b| (b.is_active() || b.is_warming()) && !used_addrs.contains(&b.addr))
                .min_by_key(|b| b.connections.load(Ordering::Relaxed))
                .cloned()
        };

        let mut slots_to_rotate: Vec<(usize, Instant)> = Vec::new();
        for (idx, slot) in slots.iter().enumerate() {
            if now.duration_since(slot.last_switch) >= slot.interval {
                slots_to_rotate.push((idx, now));
            }
        }

        for (idx, _) in slots_to_rotate {
            let used_addrs: std::collections::HashSet<_> = slots.iter().map(|s| s.backend.addr).collect();
            if let Some(new_b) = get_active_unused(&used_addrs) {
                slots[idx].backend = new_b;
                slots[idx].last_switch = now;
            }
        }

        let total_conns: usize = slots.iter().map(|s| s.backend.connections.load(Ordering::Relaxed)).sum();
        let last_expand = self.last_expand.lock();
        let should_expand = slots.is_empty() || (
            slots.len() < MAX_STICKY_SLOTS && (
                now.duration_since(*last_expand) >= Duration::from_secs(10) ||
                slots.len() * slots.len() < total_conns
            )
        );
        drop(last_expand);

        if should_expand {
            let used_addrs: std::collections::HashSet<_> = slots.iter().map(|s| s.backend.addr).collect();
            if let Some(b) = get_active_unused(&used_addrs) {
                let interval = Duration::from_secs(STICKY_BASE_SECS + (slots.len() as u64 * STICKY_INCREMENT_SECS));
                slots.push(StickySlot {
                    backend: b,
                    last_switch: now,
                    last_access: now,
                    interval,
                });
                *self.last_expand.lock() = now;
            }
        }

        if slots.is_empty() {
            return None;
        }

        let len = slots.len();
        let selected = if len == 1 {
            &mut slots[0]
        } else {
            let i = index.fetch_add(1, Ordering::Relaxed) % len;
            let j = (i + 1) % len;
            if slots[i].backend.connections.load(Ordering::Relaxed) <= slots[j].backend.connections.load(Ordering::Relaxed) {
                &mut slots[i]
            } else {
                &mut slots[j]
            }
        };

        selected.last_access = now;
        selected.backend.connections.fetch_add(1, Ordering::Relaxed);
        Some(selected.backend.clone())
    }
    
    fn check_warming_backends(&self, pool: &mut Vec<Arc<Backend>>) {
        for backend in pool.iter() {
            if backend.check_warming_expired() {
                backend.mark_active();
            }
        }
    }

    fn count_active(&self, pool: &[Arc<Backend>]) -> usize {
        pool.iter().filter(|b| b.is_active()).count()
    }

    fn calculate_pool_avg_delay(&self, pool: &[Arc<Backend>]) -> f32 {
        let mut total_delay = 0.0;
        let mut total_samples = 0;

        for backend in pool {
            let sample_count = backend.get_sample_count();
            if sample_count > 0 {
                let avg_delay = backend.get_avg_delay();
                total_delay += avg_delay * sample_count as f32;
                total_samples += sample_count;
            }
        }

        if total_samples == 0 {
            1.0
        } else {
            total_delay / total_samples as f32
        }
    }

    fn calculate_pool_avg_loss(&self, pool: &[Arc<Backend>]) -> f32 {
        let mut total_loss = 0.0;
        let mut total_samples = 0;

        for backend in pool {
            let sc = backend.get_sample_count();
            if sc > 0 {
                total_loss += backend.get_loss_rate() * sc as f32;
                total_samples += sc;
            }
        }

        if total_samples > 0 {
            total_loss / total_samples as f32
        } else {
            0.0
        }
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
        
        if sample_count < EVICT_THRESHOLD {
            return false;
        }
        
        let backup = self.backup.read();
        let active_count = self.count_active(&backup);
        drop(backup);
        
        if active_count <= self.min_active_target {
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
        
        let pool_avg_delay = self.calculate_pool_avg_delay(&self.backup.read());
        let pool_avg_loss = self.calculate_pool_avg_loss(&self.backup.read());
        
        let backend = Arc::new(Backend::new_with_initial(addr, pool_avg_delay, pool_avg_loss));
        self.backup.write().push(backend);
        self.ip_set.write().insert(ip);
    }

    pub(crate) fn get_backup_backends(&self) -> Vec<Arc<Backend>> {
        self.backup.read().clone()
    }

    fn sort_backup(&self, pool_avg_delay: f32, pool_avg_loss: f32) {
        let mut backup = self.backup.write();
        backup.sort_by(|a, b| {
            let score_a = a.calculate_score(pool_avg_delay, pool_avg_loss);
            let score_b = b.calculate_score(pool_avg_delay, pool_avg_loss);
            score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
        });
        self.backup_index.store(0, Ordering::Relaxed);
    }

    pub(crate) fn start_health_check(self: Arc<Self>) {
        let client = match &self.client {
            Some(c) => c.clone(),
            None => return,
        };

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(HEALTH_CHECK_INTERVAL_SECS));
            
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

                {
                    let mut backup = self.backup.write();
                    self.check_warming_backends(&mut backup);
                }

                let backends = self.get_backup_backends();
                
                if backends.is_empty() {
                    continue;
                }

                println!("[健康检查] 开始并发检查 {} 个备选 IP ← 每{}秒主动测速", backends.len(), HEALTH_CHECK_INTERVAL_SECS);

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
                            if let Some(res) = join_set.join_next().await
                                && let Ok((backend, result, delay_threshold, loss_threshold, lb)) = res
                            {
                                Self::handle_health_check_result(backend, result, delay_threshold, loss_threshold, lb);
                            }
                        }
                    }
                }

                while let Some(res) = join_set.join_next().await
                    && let Ok((backend, result, delay_threshold, loss_threshold, lb)) = res
                {
                    Self::handle_health_check_result(backend, result, delay_threshold, loss_threshold, lb);
                }

                self.cleanup_removed();
                
                let pool_avg_delay = self.calculate_pool_avg_delay(&self.backup.read());
                let pool_avg_loss = self.calculate_pool_avg_loss(&self.backup.read());
                self.sort_backup(pool_avg_delay, pool_avg_loss);
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
        let state_str = if backend.is_warming() {
            "预热中"
        } else if backend.is_active() {
            "正常"
        } else {
            "已移除"
        };

        match result {
            Some((_addr, delay, colo, success_count)) => {
                backend.record_delay(delay);
                
                let is_loss = success_count < PING_TIMES;
                backend.record_loss(is_loss);
                
                let sample_count = backend.get_sample_count();
                let colo_str = colo.as_deref().unwrap_or("???");
                
                if sample_count < SAMPLE_WINDOW as usize {
                    println!("[健康检查] {} 延迟 {:.1}ms (成功{}/{}) [{}] 收集中 {}/{} [{}] ← 每{}秒主动测速", 
                        backend.addr, delay, success_count, PING_TIMES, colo_str, sample_count, SAMPLE_WINDOW as usize, state_str, HEALTH_CHECK_INTERVAL_SECS);
                    return;
                }
                
                let avg_delay = backend.get_avg_delay();
                let loss_rate = backend.get_loss_rate();
                
                if avg_delay > delay_threshold {
                    println!("[剔除] {} 延迟 {:.1}ms/阈值 {:.1}ms 丢包率 {:.1}% [{}] ← 每{}秒主动测速", 
                        backend.addr, avg_delay, delay_threshold, loss_rate * 100.0, colo_str, HEALTH_CHECK_INTERVAL_SECS);
                    lb.remove_backend(backend);
                } else {
                    println!("[健康检查] {} 延迟 {:.1}ms (成功{}/{}) [{}] [{}] ← 每{}秒主动测速", 
                        backend.addr, delay, success_count, PING_TIMES, colo_str, state_str, HEALTH_CHECK_INTERVAL_SECS);
                }
            }
            None => {
                backend.record_loss(true);
                
                let sample_count = backend.get_sample_count();
                if sample_count < SAMPLE_WINDOW as usize {
                    println!("[健康检查] {} 测速失败 (成功0/{}) 收集中 {}/{} [{}] ← 每{}秒主动测速", 
                        backend.addr, PING_TIMES, sample_count, SAMPLE_WINDOW as usize, state_str, HEALTH_CHECK_INTERVAL_SECS);
                    return;
                }
                
                let loss_rate = backend.get_loss_rate();
                let avg_delay = backend.get_avg_delay();
                if loss_rate > loss_threshold {
                    println!("[剔除] {} 丢包率 {:.1}%/阈值 {:.1}% 延迟 {:.1}ms ← 每{}秒主动测速", 
                        backend.addr, loss_rate * 100.0, loss_threshold * 100.0, avg_delay, HEALTH_CHECK_INTERVAL_SECS);
                    lb.remove_backend(backend);
                } else {
                    println!("[健康检查] {} 测速失败 (成功0/{}) [{}] ← 每{}秒主动测速", backend.addr, PING_TIMES, state_str, HEALTH_CHECK_INTERVAL_SECS);
                }
            }
        }
    }
}