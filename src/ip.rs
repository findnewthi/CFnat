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

enum IpSource {
    Single {
        ip: IpAddr,
        consumed: AtomicBool,
    },
    Cidr {
        start: u128,
        end: u128,
        current: AtomicU64,
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
            IpSource::Cidr { start, end, current, is_v6 } => {
                let idx = current.fetch_add(1, Ordering::Relaxed) as u128;
                if idx >= *end - *start {
                    return None;
                }
                let ip_val = *start + idx;
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
            IpSource::Cidr { start, end, current, is_v6: _ } => {
                let idx = current.load(Ordering::Relaxed) as u128;
                idx >= *end - *start
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
        let mut total: u128 = 0;

        for source in sources {
            let s = source.trim();
            if s.is_empty() || s.starts_with('#') || s.starts_with("//") {
                continue;
            }

            if let Ok(ip) = s.parse::<IpAddr>() {
                single_ips.push(ip);
                total += 1;
            } else if let Some(cidr) = IpCidr::parse(s) {
                let (start, end) = cidr.range_u128();
                let count = end - start + 1;
                cidr_sources.push(Arc::new(IpSource::Cidr {
                    start,
                    end: end + 1,
                    current: AtomicU64::new(0),
                    is_v6: matches!(cidr, IpCidr::V6(_, _)),
                }));
                total += count;
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
            total_count: AtomicU64::new(total as u64),
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