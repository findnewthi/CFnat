use std::net::SocketAddr;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::time::Instant;

use parking_lot::Mutex;

use crate::core::config::get_global_config;
use crate::core::utils;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum BackendState {
    Warming = 0,
    Active = 1,
    Removed = 2,
}

pub struct Backend {
    pub addr: SocketAddr,
    pub colo: Mutex<Option<String>>,
    connections: AtomicUsize,
    avg_delay: Mutex<f32>,
    avg_loss: Mutex<f32>,
    sample_count: AtomicUsize,
    state: AtomicU8,
    entered_state_at: Mutex<Instant>,
}

impl Backend {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            colo: Mutex::new(None),
            connections: AtomicUsize::new(0),
            avg_delay: Mutex::new(-1.0),
            avg_loss: Mutex::new(-1.0),
            sample_count: AtomicUsize::new(0),
            state: AtomicU8::new(BackendState::Warming as u8),
            entered_state_at: Mutex::new(Instant::now()),
        }
    }

    pub fn new_with_initial(addr: SocketAddr, initial_delay: f32, initial_loss: f32, colo: Option<String>) -> Self {
        Self {
            addr,
            colo: Mutex::new(colo),
            connections: AtomicUsize::new(0),
            avg_delay: Mutex::new(initial_delay),
            avg_loss: Mutex::new(initial_loss),
            sample_count: AtomicUsize::new(0),
            state: AtomicU8::new(BackendState::Warming as u8),
            entered_state_at: Mutex::new(Instant::now()),
        }
    }

    pub fn set_colo(&self, colo: Option<String>) {
        *self.colo.lock() = colo;
    }

    pub fn get_colo(&self) -> Option<String> {
        self.colo.lock().clone()
    }

    fn update_ewma(current: &mut f32, new_val: f32, is_first: bool) {
        utils::update_ewma(current, new_val, is_first, get_global_config().alpha);
    }

    pub fn record_delay(&self, delay_ms: f32) {
        let is_first = self.sample_count.load(Ordering::Relaxed) == 0;
        let mut lock = self.avg_delay.lock();
        Self::update_ewma(&mut lock, delay_ms, is_first);
        
        let _ = self.sample_count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            Some((count + 1).min(get_global_config().sample_window as usize))
        });
    }

    pub fn record_loss(&self, is_loss: bool) {
        let is_first = self.sample_count.load(Ordering::Relaxed) == 0;
        let mut lock = self.avg_loss.lock();
        let loss = if is_loss { 1.0 } else { 0.0 };
        Self::update_ewma(&mut lock, loss, is_first);
    }

    pub fn get_avg_delay(&self) -> f32 {
        self.avg_delay.lock().max(0.0)
    }

    pub fn get_loss_rate(&self) -> f32 {
        self.avg_loss.lock().max(0.0)
    }

    pub fn get_sample_count(&self) -> usize {
        self.sample_count.load(Ordering::Relaxed)
    }

    pub fn is_removed(&self) -> bool {
        self.state.load(Ordering::Relaxed) == BackendState::Removed as u8
    }

    pub fn is_warming(&self) -> bool {
        self.state.load(Ordering::Relaxed) == BackendState::Warming as u8
    }

    pub fn is_active(&self) -> bool {
        self.state.load(Ordering::Relaxed) == BackendState::Active as u8
    }

    pub fn mark_removed(&self) {
        self.state.store(BackendState::Removed as u8, Ordering::Relaxed);
        *self.entered_state_at.lock() = Instant::now();
    }

    pub fn mark_active(&self) {
        self.state.store(BackendState::Active as u8, Ordering::Relaxed);
        *self.entered_state_at.lock() = Instant::now();
    }

    pub fn check_warming_expired(&self) -> bool {
        if self.is_warming() {
            let elapsed = self.entered_state_at.lock().elapsed().as_secs();
            elapsed >= get_global_config().warming_duration.as_secs()
        } else {
            false
        }
    }

    pub fn calculate_score(&self, pool_avg_delay: f32, pool_avg_loss: f32) -> f32 {
        let connections = self.connections.load(Ordering::Relaxed) as f32;
        let beta = (self.get_sample_count() as f32 / get_global_config().sample_window).min(1.0);
        
        let my_perf = self.get_avg_delay() * (1.0 + self.get_loss_rate() * 2.0);
        let pool_perf = (pool_avg_delay * (1.0 + pool_avg_loss * 2.0)).max(1.0);
        
        let ratio = if pool_perf > 0.0 { my_perf / pool_perf } else { 1.0 };
        let smooth_ratio = (1.0 + beta * (ratio - 1.0)).min(get_global_config().max_smooth_ratio);
        
        (connections + 1.0) * smooth_ratio
    }

    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::Relaxed)
    }

    pub fn fetch_add_connection(&self, val: usize) {
        self.connections.fetch_add(val, Ordering::Relaxed);
    }

    pub fn fetch_sub_connection(&self, val: usize) {
        self.connections.fetch_sub(val, Ordering::Relaxed);
    }
}