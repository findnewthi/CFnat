use std::fs::File;
use std::io::{self, BufRead};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy)]
pub(crate) enum IpCidr {
    V4(Ipv4Addr, u8),
}

impl IpCidr {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return None;
        }
        let ip = parts[0].parse::<Ipv4Addr>().ok()?;
        let prefix = parts[1].parse::<u8>().ok()?;
        if prefix <= 32 {
            Some(IpCidr::V4(ip, prefix))
        } else {
            None
        }
    }

    pub(crate) fn range(&self) -> (u32, u32) {
        match self {
            IpCidr::V4(ip, len) => {
                let val = u32::from(*ip);
                let host_bits = 32 - len;
                let mask = if host_bits >= 32 { 0 } else { !0u32 << host_bits };
                let start = val & mask;
                let end = start | (!mask);
                (start, end)
            }
        }
    }
}

pub(crate) struct IpPool {
    ranges: Vec<(u32, u32)>,
    cursor: AtomicU64,
    total_count: u64,
}

impl IpPool {
    pub(crate) fn new(sources: &[String]) -> Self {
        let mut ranges = Vec::new();
        
        for source in sources {
            let s = source.trim();
            if s.is_empty() || s.starts_with('#') || s.starts_with("//") {
                continue;
            }
            
            if let Ok(ip) = s.parse::<IpAddr>() {
                let val = match ip {
                    IpAddr::V4(v4) => u32::from(v4),
                    IpAddr::V6(_) => continue,
                };
                ranges.push((val, val));
            } else if let Some(cidr) = IpCidr::parse(s) {
                ranges.push(cidr.range());
            }
        }
        
        let total_count: u64 = ranges.iter()
            .map(|(s, e)| (*e - *s + 1) as u64)
            .sum();
        
        Self { ranges, cursor: AtomicU64::new(0), total_count }
    }

    pub(crate) fn from_file(path: &str) -> Self {
        let mut sources = Vec::new();
        
        if let Ok(file) = File::open(path) {
            for line in io::BufReader::new(file).lines().flatten() {
                sources.push(line);
            }
        }
        
        Self::new(&sources)
    }

    pub(crate) fn total_count(&self) -> u64 {
        self.total_count
    }

    pub(crate) fn pop(&self) -> Option<IpAddr> {
        if self.total_count == 0 {
            return None;
        }
        
        let idx = self.cursor.fetch_add(1, Ordering::Relaxed);
        let idx = idx % self.total_count;
        
        let mut offset = idx;
        for (start, end) in &self.ranges {
            let len = (end - start + 1) as u64;
            if offset < len {
                let ip = Ipv4Addr::from(start + offset as u32);
                return Some(IpAddr::V4(ip));
            }
            offset -= len;
        }
        
        None
    }
}