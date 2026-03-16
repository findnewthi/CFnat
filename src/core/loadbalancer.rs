use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use parking_lot::RwLock;
use tokio_util::sync::CancellationToken;

use crate::core::backend::Backend;
use crate::core::config::get_global_config;
use crate::core::httping::{PingConfig, PingResultDetail};
use crate::core::utils;

struct StickySlot {
    backend: Arc<Backend>,
    last_switch: Instant,
    last_access: Instant,
    interval: Duration,
}

#[derive(Clone)]
struct HealthCheckConfig {
    ping_config: PingConfig,
    delay_threshold: f32,
    loss_threshold: f32,
}

pub struct LoadBalancer {
    primary: RwLock<Vec<Arc<Backend>>>,
    primary_index: AtomicUsize,
    backup: RwLock<Vec<Arc<Backend>>>,
    backup_index: AtomicUsize,
    ip_set: RwLock<HashSet<std::net::IpAddr>>,
    primary_target: AtomicUsize,
    backup_target: AtomicUsize,
    min_active_target: usize,
    health_check_url: String,
    tls_port: u16,
    http_port: u16,
    timeout_ms: u64,
    notify_tx: Option<tokio::sync::watch::Sender<bool>>,
    delay_threshold: RwLock<f32>,
    loss_threshold: RwLock<f32>,
    client: Option<Arc<crate::core::hyper::MyHyperClient>>,
    colo_filter: Option<Arc<Vec<String>>>,
    sticky_slots: Mutex<Vec<StickySlot>>,
    last_expand: Mutex<Instant>,
    cancel_token: CancellationToken,
    next_health_check: Mutex<Instant>,
    next_primary_health_check: Mutex<Instant>,
}

pub enum AddResult {
    AddedToPrimary,
    AddedToBackup,
    QueueFull,
    AlreadyExists,
}

impl LoadBalancer {
    pub fn new(primary_target: usize) -> Self {
        let backup_target = ((primary_target as f32 * 0.5).ceil() as usize).min(get_global_config().max_backup_target).max(2);
        let min_active_target = (primary_target as f32 / 2.0).ceil() as usize;
        Self {
            primary: RwLock::new(Vec::new()),
            primary_index: AtomicUsize::new(0),
            backup: RwLock::new(Vec::new()),
            backup_index: AtomicUsize::new(0),
            ip_set: RwLock::new(HashSet::new()),
            primary_target: AtomicUsize::new(primary_target),
            backup_target: AtomicUsize::new(backup_target),
            min_active_target,
            health_check_url: String::new(),
            tls_port: 443,
            http_port: 80,
            timeout_ms: 2000,
            notify_tx: None,
            delay_threshold: RwLock::new(0.0),
            loss_threshold: RwLock::new(0.0),
            client: None,
            colo_filter: None,
            sticky_slots: Mutex::new(Vec::new()),
            last_expand: Mutex::new(Instant::now()),
            cancel_token: CancellationToken::new(),
            next_health_check: Mutex::new(Instant::now()),
            next_primary_health_check: Mutex::new(Instant::now()),
        }
    }

    pub fn try_add_backend(
        &self,
        addr: SocketAddr,
        delay: f32,
        colo: Option<&str>,
    ) -> AddResult {
        if self.contains(addr.ip()) {
            return AddResult::AlreadyExists;
        }
        
        let primary_count = self.get_primary_count();
        let backup_count = self.get_backup_count();
        
        let primary_target = self.primary_target.load(Ordering::Relaxed);
        let backup_target = self.backup_target.load(Ordering::Relaxed);
        
        let colo_string = colo.map(|s| s.to_string());
        
        if primary_count < primary_target {
            self.add_to_primary(addr, delay, 0.0, colo_string);
            println!("[+] {} {:.0}ms [{}]", addr, delay, colo.unwrap_or(""));
            AddResult::AddedToPrimary
        } else if backup_count < backup_target {
            self.add_to_backup(addr, delay, 0.0, colo_string);
            println!("[+] {} {:.0}ms [{}] (备选)", addr, delay, colo.unwrap_or(""));
            AddResult::AddedToBackup
        } else {
            AddResult::QueueFull
        }
    }

    pub fn with_delay_threshold(self, delay_threshold: f32) -> Self {
        *self.delay_threshold.write() = delay_threshold;
        self
    }

    pub fn with_loss_threshold(self, loss_threshold: f32) -> Self {
        *self.loss_threshold.write() = loss_threshold;
        self
    }

    pub fn with_colo_filter(mut self, colo_filter: Option<Vec<String>>) -> Self {
        self.colo_filter = colo_filter.map(Arc::new);
        self
    }

    pub fn get_delay_threshold(&self) -> f32 {
        *self.delay_threshold.read()
    }

    pub fn with_health_check_url(mut self, url: String) -> Self {
        self.health_check_url = url;
        self
    }

    pub fn with_ports(mut self, tls_port: u16, http_port: u16) -> Self {
        self.tls_port = tls_port;
        self.http_port = http_port;
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn with_notify(mut self, tx: tokio::sync::watch::Sender<bool>) -> Self {
        self.notify_tx = Some(tx);
        self
    }

    pub fn with_client(mut self, client: Arc<crate::core::hyper::MyHyperClient>) -> Self {
        self.client = Some(client);
        self
    }

    pub fn contains(&self, ip: std::net::IpAddr) -> bool {
        self.ip_set.read().contains(&ip)
    }

    pub fn primary_full(&self) -> bool {
        self.get_primary_count() >= self.primary_target.load(Ordering::Relaxed)
    }

    pub fn backup_full(&self) -> bool {
        self.get_backup_count() >= self.backup_target.load(Ordering::Relaxed)
    }

    pub fn should_pause(&self) -> bool {
        self.primary_full() && self.backup_full()
    }

    pub fn notify_resume(&self) {
        if let Some(tx) = &self.notify_tx {
            let _ = tx.send(true);
        }
    }

    pub fn select(&self) -> Option<Arc<Backend>> {
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

        slots.retain(|s| now.duration_since(s.last_access) < get_global_config().sticky_slot_ttl);

        let get_active_unused = |used_addrs: &std::collections::HashSet<SocketAddr>| {
            pool.iter()
                .filter(|b| (b.is_active() || b.is_warming()) && !used_addrs.contains(&b.addr))
                .min_by_key(|b| b.connections())
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

        let total_conns: usize = slots.iter().map(|s| s.backend.connections()).sum();
        let last_expand = self.last_expand.lock();
        let should_expand = slots.is_empty() || (
            slots.len() < get_global_config().max_sticky_slots && (
                now.duration_since(*last_expand) >= get_global_config().sticky_slot_expand_interval ||
                slots.len() * slots.len() < total_conns
            )
        );
        drop(last_expand);

        if should_expand {
            let used_addrs: std::collections::HashSet<_> = slots.iter().map(|s| s.backend.addr).collect();
            if let Some(b) = get_active_unused(&used_addrs) {
                let interval = get_global_config().sticky_base_interval + Duration::from_secs((slots.len() as u64) * get_global_config().sticky_increment_interval.as_secs());
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
            if slots[i].backend.connections() <= slots[j].backend.connections() {
                &mut slots[i]
            } else {
                &mut slots[j]
            }
        };

        selected.last_access = now;
        selected.backend.fetch_add_connection(1);
        Some(selected.backend.clone())
    }
    
    fn check_warming_backends(&self, pool: &mut [Arc<Backend>]) {
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
        utils::calculate_pool_avg_delay(pool)
    }

    fn calculate_pool_avg_loss(&self, pool: &[Arc<Backend>]) -> f32 {
        utils::calculate_pool_avg_loss(pool)
    }

    fn cleanup_removed(&self) {
        let mut primary = self.primary.write();
        let before = primary.len();
        primary.retain(|b| !b.is_removed());
        let primary_removed = before - primary.len();
        drop(primary);

        let mut backup = self.backup.write();
        let backup_before = backup.len();
        backup.retain(|b| !b.is_removed());
        let backup_removed = backup_before - backup.len();
        drop(backup);

        let removed_count = primary_removed + backup_removed;
        if removed_count > 0 {
            println!("[-] 清理 {} 个失效节点", removed_count);
        }
    }

    pub fn release(&self, backend: &Backend) {
        backend.fetch_sub_connection(1);
    }

    pub fn record_delay(&self, backend: &Backend, delay_ms: f32) {
        backend.record_delay(delay_ms);
    }

    pub fn record_loss(&self, backend: &Backend, is_loss: bool) {
        backend.record_loss(is_loss);
    }

    pub fn check_and_evict(&self, backend: &Backend) -> bool {
        let sample_count = backend.get_sample_count();
        
        if sample_count < get_global_config().evict_threshold {
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

        let delay_threshold = *self.delay_threshold.read();
        let loss_threshold = *self.loss_threshold.read();

        if avg_delay > delay_threshold || loss_rate > loss_threshold {
            backend.mark_removed();
            true
        } else {
            false
        }
    }

    pub fn remove_backend(&self, backend: Arc<Backend>) {
        let ip = backend.addr.ip();
        
        self.primary.write().retain(|b| b.addr.ip() != ip);
        self.backup.write().retain(|b| b.addr.ip() != ip);
        self.ip_set.write().remove(&ip);
        self.notify_resume();
    }

    pub fn refill_from_backup(&self) {
        let primary_len = {
            self.primary.read().len()
        };

        let primary_target = self.primary_target.load(Ordering::Relaxed);

        if primary_len < primary_target {
            let mut backup = self.backup.write();
            let mut primary = self.primary.write();
            let mut promoted = 0;
            while primary.len() < primary_target && !backup.is_empty() {
                let backend = backup.remove(0);
                primary.push(backend);
                promoted += 1;
            }
            if promoted > 0 {
                println!("[↑] {} 个备选提升到主队列", promoted);
            }
        }

        self.notify_resume();
    }

    pub fn get_backup_count(&self) -> usize {
        self.backup.read().len()
    }

    pub fn get_primary_count(&self) -> usize {
        self.primary.read().len()
    }

    pub fn get_primary_target(&self) -> usize {
        self.primary_target.load(Ordering::Relaxed)
    }

    pub fn get_backup_target(&self) -> usize {
        self.backup_target.load(Ordering::Relaxed)
    }

    pub fn update_delay_threshold(&self, delay_threshold: f32) {
        let mut delay = self.delay_threshold.write();
        *delay = delay_threshold;
    }

    pub fn update_loss_threshold(&self, loss_threshold: f32) {
        let mut loss = self.loss_threshold.write();
        *loss = loss_threshold;
    }

    pub fn update_primary_target(&self, primary_target: usize) {
        self.primary_target.store(primary_target, Ordering::Relaxed);
        let backup_target = ((primary_target as f32 * 0.5).ceil() as usize).min(get_global_config().max_backup_target).max(2);
        self.backup_target.store(backup_target, Ordering::Relaxed);
    }

    pub fn add_to_primary(&self, addr: SocketAddr, initial_delay: f32, initial_loss: f32, colo: Option<String>) {
        let ip = addr.ip();
        if self.contains(ip) {
            return;
        }
        
        let backend = Arc::new(Backend::new_with_initial(addr, initial_delay, initial_loss, colo));
        self.primary.write().push(backend);
        self.ip_set.write().insert(ip);
    }

    pub fn add_to_backup(&self, addr: SocketAddr, initial_delay: f32, initial_loss: f32, colo: Option<String>) {
        let ip = addr.ip();
        if self.contains(ip) {
            return;
        }
        
        let backend = Arc::new(Backend::new_with_initial(addr, initial_delay, initial_loss, colo));
        self.backup.write().push(backend);
        self.ip_set.write().insert(ip);
    }

    pub fn get_primary_backends(&self) -> Vec<Arc<Backend>> {
        self.primary.read().clone()
    }

    pub fn get_backup_backends(&self) -> Vec<Arc<Backend>> {
        self.backup.read().clone()
    }

    pub fn get_sticky_ips(&self) -> Vec<std::net::IpAddr> {
        self.sticky_slots
            .lock()
            .iter()
            .map(|s| s.backend.addr.ip())
            .collect()
    }

    pub fn stop(&self) {
        self.cancel_token.cancel();
    }

    pub fn get_next_health_check_secs(&self) -> u64 {
        let next = *self.next_health_check.lock();
        let now = Instant::now();
        if next > now {
            (next - now).as_secs()
        } else {
            0
        }
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

    pub fn start_health_check(self: Arc<Self>) {
        let Some(client) = &self.client else { return };
        let client = client.clone();

        let cancel_token = self.cancel_token.clone();

        let Some((_, host, scheme, path)) = crate::core::hyper::parse_url(&self.health_check_url) else { return };
        
        let health_check_config = HealthCheckConfig {
            ping_config: PingConfig {
                tls_port: self.tls_port,
                http_port: self.http_port,
                client,
                host: Arc::from(host.as_str()),
                scheme: Arc::from(scheme),
                path: Arc::from(path.as_str()),
                timeout_ms: self.timeout_ms,
                colo_filter: self.colo_filter.clone(),
            },
            delay_threshold: *self.delay_threshold.read(),
            loss_threshold: *self.loss_threshold.read(),
        };

        *self.next_health_check.lock() = Instant::now() + get_global_config().health_check_interval;
        *self.next_primary_health_check.lock() = Instant::now() + Duration::from_secs(120);

        let lb = self.clone();
        tokio::spawn(async move {
            let mut backup_interval = tokio::time::interval(get_global_config().health_check_interval);
            backup_interval.tick().await;
            
            let mut primary_interval = tokio::time::interval(Duration::from_secs(120));
            primary_interval.tick().await;

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        println!("[健康检查] 收到停止信号，退出");
                        break;
                    }
                    _ = backup_interval.tick() => {
                        lb.run_backup_health_check(health_check_config.clone());
                    }
                    _ = primary_interval.tick() => {
                        lb.run_primary_health_check(health_check_config.clone());
                    }
                }
            }
        });
    }

    fn run_health_check_for_pool(
        self: &Arc<Self>,
        config: HealthCheckConfig,
        source: &str,
        interval: Duration,
    ) {
        let (backends, is_primary) = if source == "主队列" {
            {
                let mut primary = self.primary.write();
                self.check_warming_backends(&mut primary);
            }
            (self.get_primary_backends(), true)
        } else {
            {
                let mut backup = self.backup.write();
                self.check_warming_backends(&mut backup);
            }
            (self.get_backup_backends(), false)
        };
        
        if backends.is_empty() {
            if is_primary {
                *self.next_primary_health_check.lock() = Instant::now() + interval;
            } else {
                *self.next_health_check.lock() = Instant::now() + interval;
            }
            return;
        }

        let lb = self.clone();
        let source_owned = source.to_string();
        let concurrency = get_global_config().health_check_concurrency;
        
        tokio::spawn(async move {
            let mut join_set = tokio::task::JoinSet::new();
            let mut removed_count = 0usize;

            for backend in backends {
                if backend.is_removed() {
                    continue;
                }

                let config = config.clone();
                let lb = lb.clone();
                let src = source_owned.clone();

                join_set.spawn(async move {
                    let result = crate::core::httping::http_ping_multi(
                        backend.addr.ip(),
                        &config.ping_config,
                    ).await;
                    (backend, result, config.delay_threshold, config.loss_threshold, lb, src)
                });

                if join_set.len() >= concurrency
                    && let Some(res) = join_set.join_next().await
                    && let Ok((backend, result, delay_threshold, loss_threshold, lb, src)) = res
                    && Self::handle_health_check_result(backend, result, delay_threshold, loss_threshold, lb, &src)
                {
                    removed_count += 1;
                }
            }

            while let Some(res) = join_set.join_next().await
                && let Ok((backend, result, delay_threshold, loss_threshold, lb, src)) = res
                && Self::handle_health_check_result(backend, result, delay_threshold, loss_threshold, lb, &src)
            {
                removed_count += 1;
            }

            lb.cleanup_removed();
            
            if removed_count > 0 {
                println!("[{}] 检查完成，移除 {} 个", source_owned, removed_count);
            }
            
            if is_primary {
                lb.refill_from_backup();
                *lb.next_primary_health_check.lock() = Instant::now() + interval;
            } else {
                let pool_avg_delay = lb.calculate_pool_avg_delay(&lb.backup.read());
                let pool_avg_loss = lb.calculate_pool_avg_loss(&lb.backup.read());
                lb.sort_backup(pool_avg_delay, pool_avg_loss);
                *lb.next_health_check.lock() = Instant::now() + interval;
            }
        });
    }

    fn run_backup_health_check(self: &Arc<Self>, config: HealthCheckConfig) {
        self.run_health_check_for_pool(config, "备选", get_global_config().health_check_interval);
    }

    fn run_primary_health_check(self: &Arc<Self>, config: HealthCheckConfig) {
        self.run_health_check_for_pool(config, "主队列", Duration::from_secs(120));
    }

    fn handle_health_check_result(
        backend: Arc<Backend>,
        result: Option<PingResultDetail>,
        delay_threshold: f32,
        loss_threshold: f32,
        lb: Arc<LoadBalancer>,
        source: &str,
    ) -> bool {
        let remove_and_refill = |backend: Arc<Backend>| {
            lb.remove_backend(backend);
            if source == "主队列" {
                lb.refill_from_backup();
            }
        };

        match result {
            Some(detail) => {
                if detail.colo_mismatch {
                    let colo_str = detail.colo.as_deref().unwrap_or("未知");
                    println!("[-] {} 数据中心[{}]不匹配", backend.addr, colo_str);
                    remove_and_refill(backend);
                    return true;
                }

                backend.record_delay(detail.delay);
                if let Some(c) = detail.colo {
                    backend.set_colo(Some(c));
                }
                
                let is_loss = detail.success_count < get_global_config().ping_times;
                backend.record_loss(is_loss);
                
                let sample_count = backend.get_sample_count();
                if sample_count < get_global_config().sample_window as usize {
                    return false;
                }
                
                let avg_delay = backend.get_avg_delay();
                let loss_rate = backend.get_loss_rate();
                
                if avg_delay > delay_threshold {
                    println!("[-] {} 延迟{:.0}ms>{:.0}ms", backend.addr, avg_delay, delay_threshold);
                    remove_and_refill(backend);
                    true
                } else if loss_rate > loss_threshold {
                    println!("[-] {} 丢包{:.0}%>{:.0}%", backend.addr, loss_rate * 100.0, loss_threshold * 100.0);
                    remove_and_refill(backend);
                    true
                } else {
                    false
                }
            }
            None => {
                backend.record_loss(true);
                
                let sample_count = backend.get_sample_count();
                if sample_count < get_global_config().sample_window as usize {
                    return false;
                }
                
                let loss_rate = backend.get_loss_rate();
                if loss_rate > loss_threshold {
                    println!("[-] {} 丢包{:.0}%>{:.0}%", backend.addr, loss_rate * 100.0, loss_threshold * 100.0);
                    remove_and_refill(backend);
                    true
                } else {
                    false
                }
            }
        }
    }
}