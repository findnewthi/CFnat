use std::fs::File;
use std::io::{self, BufRead};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) enum IpCidr {
    V4(Ipv4Addr, u8),
    V6(Ipv6Addr, u8),
}

impl IpCidr {
    fn parts(&self) -> (u128, u8, u8, u128) {
        match self {
            IpCidr::V4(ip, len) => (u32::from(*ip) as u128, *len, 32, u32::MAX as u128),
            IpCidr::V6(ip, len) => (u128::from(*ip), *len, 128, u128::MAX),
        }
    }

    pub(crate) fn range_u128(&self) -> (u128, u128) {
        let (val, len, max_bits, full_mask) = self.parts();
        let host_bits = max_bits - len;

        if host_bits >= max_bits {
            return (0, full_mask);
        }

        let mask = full_mask << host_bits & full_mask;
        let start = val & mask;
        let end = start | (!mask & full_mask);
        
        (start, end)
    }

    pub(crate) fn prefix_len(&self) -> u8 {
        match self {
            IpCidr::V4(_, len) | IpCidr::V6(_, len) => *len,
        }
    }

    pub(crate) fn is_single_host(&self) -> bool {
        matches!(self, IpCidr::V4(_, 32) | IpCidr::V6(_, 128))
    }

    pub(crate) fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return None;
        }

        let ip = IpAddr::from_str(parts[0]).ok()?;
        let prefix = parts[1].parse::<u8>().ok()?;

        match ip {
            IpAddr::V4(v4) if prefix <= 32 => Some(IpCidr::V4(v4, prefix)),
            IpAddr::V6(v6) if prefix <= 128 => Some(IpCidr::V6(v6, prefix)),
            _ => None,
        }
    }
}

const DEFAULT_SAMPLES_PER_SUBNET: u128 = 5;

fn generate_refined_random(obj_addr: usize) -> u128 {
    static SHARED_STATE: AtomicUsize = AtomicUsize::new(0);

    let hasher_seed = generate_refined_random as *const () as usize;

    let s = SHARED_STATE.fetch_add(1, Ordering::Relaxed);

    let t = &s as *const _ as usize;
    let mut x = s ^ obj_addr ^ t;

    x = x.wrapping_mul(hasher_seed | 1);

    x = x.rotate_left((usize::BITS / 2) as u32);
    x = x.swap_bytes();

    x as u128
}

fn calculate_sample_count(prefix: u8, is_ipv4: bool) -> u128 {
    let threshold: u8 = if is_ipv4 { 24 } else { 48 };
    let shift = u32::from(threshold.saturating_sub(prefix));
    (1u128 << shift).saturating_mul(DEFAULT_SAMPLES_PER_SUBNET)
}

enum IpSource {
    Single {
        ip: IpAddr,
        consumed: AtomicBool,
    },
    Cidr {
        start: u128,
        interval_size: u128,
        last_size: u128,
        total_count: u64,
        current: AtomicUsize,
        is_v6: bool,
    },
}

impl IpSource {
    fn next_ip(&self) -> Option<IpAddr> {
        match self {
            IpSource::Single { ip, consumed } => {
                if consumed.swap(true, Ordering::SeqCst) {
                    return None;
                }
                Some(*ip)
            }
            IpSource::Cidr { start, interval_size, last_size, total_count, current, is_v6 } => {
                let idx = current.fetch_add(1, Ordering::Relaxed);
                let total = *total_count as usize;
                if idx >= total {
                    return None;
                }
                
                let interval = *interval_size;
                let interval_start = *start + (idx as u128 * interval);
                let actual_interval_size = if idx == total - 1 {
                    *last_size
                } else {
                    interval
                };
                
                let random_offset = if actual_interval_size <= 1 {
                    0
                } else {
                    generate_refined_random(self as *const Self as usize) % actual_interval_size
                };
                
                let ip_val = interval_start + random_offset;
                if *is_v6 {
                    Some(IpAddr::V6(Ipv6Addr::from(ip_val)))
                } else {
                    Some(IpAddr::V4(Ipv4Addr::from(ip_val as u32)))
                }
            }
        }
    }

    fn is_exhausted(&self) -> bool {
        match self {
            IpSource::Single { consumed, .. } => {
                consumed.load(Ordering::Relaxed)
            }
            IpSource::Cidr { total_count, current, .. } => {
                current.load(Ordering::Relaxed) >= *total_count as usize
            }
        }
    }

    fn reset(&self) {
        match self {
            IpSource::Single { consumed, .. } => {
                consumed.store(false, Ordering::Relaxed);
            }
            IpSource::Cidr { current, .. } => {
                current.store(0, Ordering::Relaxed);
            }
        }
    }
}

pub(crate) struct IpPool {
    sources: Vec<Arc<IpSource>>,
    cursor: AtomicUsize,
    active_count: AtomicUsize,
    total_count: AtomicU64,
}

impl IpPool {
    pub(crate) fn new(sources: &[String]) -> Self {
        let mut single_ips = Vec::new();
        let mut cidr_sources = Vec::new();
        let mut total: u64 = 0;

        for source in sources {
            let s = source.trim();
            if s.is_empty() || s.starts_with('#') || s.starts_with("//") {
                continue;
            }

            let (cidr_part, custom_count) = if let Some((cidr_part, count_str)) = s.split_once('=') {
                let count = count_str.trim().parse::<u128>().ok().filter(|&n| n > 0);
                (cidr_part.trim(), count)
            } else {
                (s, None)
            };

            if let Ok(ip) = cidr_part.parse::<IpAddr>() {
                single_ips.push(ip);
                total += 1;
            } else if let Some(cidr) = IpCidr::parse(cidr_part) {
                if cidr.is_single_host() {
                    let ip = match cidr {
                        IpCidr::V4(v4, _) => IpAddr::V4(v4),
                        IpCidr::V6(v6, _) => IpAddr::V6(v6),
                    };
                    single_ips.push(ip);
                    total += 1;
                } else {
                    let (start, end) = cidr.range_u128();
                    let range_size = (end - start).saturating_add(1);
                    
                    let is_ipv6 = matches!(cidr, IpCidr::V6(_, _));
                    
                    let sample_count = if let Some(count) = custom_count {
                        count.min(range_size)
                    } else {
                        calculate_sample_count(cidr.prefix_len(), !is_ipv6) as u128
                    };
                    
                    let interval_size = if sample_count > 0 {
                        range_size.saturating_div(sample_count).max(1)
                    } else {
                        1
                    };
                    
                    let last_size = if sample_count > 0 {
                        let last_start = start + (sample_count - 1) * interval_size;
                        (end - last_start).saturating_add(1)
                    } else {
                        interval_size
                    };
                    
                    cidr_sources.push(Arc::new(IpSource::Cidr {
                        start,
                        interval_size,
                        last_size,
                        total_count: sample_count as u64,
                        current: AtomicUsize::new(0),
                        is_v6: is_ipv6,
                    }));
                    total += sample_count as u64;
                }
            }
        }

        let mut sources_vec: Vec<Arc<IpSource>> = Vec::new();

        const CHUNK_SIZE: usize = 1024;
        for chunk in single_ips.chunks(CHUNK_SIZE) {
            for &ip in chunk {
                sources_vec.push(Arc::new(IpSource::Single {
                    ip,
                    consumed: AtomicBool::new(false),
                }));
            }
        }

        sources_vec.extend(cidr_sources);

        let active_count = sources_vec.len();

        Self {
            sources: sources_vec,
            cursor: AtomicUsize::new(0),
            active_count: AtomicUsize::new(active_count),
            total_count: AtomicU64::new(total),
        }
    }

    pub(crate) fn from_file(path: &str) -> Self {
        let mut lines = Vec::new();
        
        if let Ok(file) = File::open(path) {
            for line in io::BufReader::new(file).lines().flatten() {
                lines.push(line);
            }
        }
        
        Self::new(&lines)
    }

    pub(crate) fn total_count(&self) -> u64 {
        self.total_count.load(Ordering::Relaxed)
    }

    pub(crate) fn pop(&self) -> Option<IpAddr> {
        loop {
            if self.active_count.load(Ordering::Acquire) == 0 {
                return None;
            }

            let start_idx = self.cursor.fetch_add(1, Ordering::Relaxed);

            for i in 0..self.sources.len() {
                let idx = (start_idx + i) % self.sources.len();
                let source = &self.sources[idx];

                if let Some(ip) = source.next_ip() {
                    return Some(ip);
                }

                if source.is_exhausted() {
                    self.active_count.fetch_sub(1, Ordering::Relaxed);
                }
            }

            for source in &self.sources {
                source.reset();
            }
            self.cursor.store(0, Ordering::Relaxed);
            self.active_count.store(self.sources.len(), Ordering::Relaxed);

            let start_idx = self.cursor.fetch_add(1, Ordering::Relaxed);
            for i in 0..self.sources.len() {
                let idx = (start_idx + i) % self.sources.len();
                let source = &self.sources[idx];
                if let Some(ip) = source.next_ip() {
                    return Some(ip);
                }
            }
        }
    }
}
