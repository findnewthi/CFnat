use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone)]
pub struct Config {
    pub sample_window: f32,
    pub alpha: f32,
    pub evict_threshold: usize,
    pub max_smooth_ratio: f32,
    pub max_backup_target: usize,
    pub ping_times: u8,
    pub health_check_concurrency: usize,
    pub health_check_interval: Duration,
    pub warming_duration: Duration,
    pub sticky_base_interval: Duration,
    pub sticky_increment_interval: Duration,
    pub max_sticky_slots: usize,
    pub sticky_slot_ttl: Duration,
    pub sticky_slot_expand_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    pub fn new() -> Self {
        Config {
            sample_window: 50.0,
            alpha: 2.0 / (50.0 + 1.0),
            evict_threshold: 20,
            max_smooth_ratio: 5.0,
            max_backup_target: 10,
            ping_times: 4,
            health_check_concurrency: 2,
            health_check_interval: Duration::from_secs(25),
            warming_duration: Duration::from_secs(60),
            sticky_base_interval: Duration::from_secs(10),
            sticky_increment_interval: Duration::from_secs(5),
            max_sticky_slots: 5,
            sticky_slot_ttl: Duration::from_secs(15),
            sticky_slot_expand_interval: Duration::from_secs(10),
        }
    }
}

pub static GLOBAL_CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get_global_config() -> &'static Config {
    GLOBAL_CONFIG.get_or_init(Config::new)
}