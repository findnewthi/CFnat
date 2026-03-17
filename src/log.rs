use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::RwLock;
use serde::Serialize;
use chrono::Local;

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub struct LogBuffer {
    logs: RwLock<VecDeque<LogEntry>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            logs: RwLock::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)),
        }
    }

    pub fn push(&self, level: &str, message: &str) {
        let entry = LogEntry {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            level: level.to_string(),
            message: message.to_string(),
        };
        
        let mut logs = self.logs.write();
        if logs.len() >= MAX_LOG_ENTRIES {
            logs.pop_front();
        }
        logs.push_back(entry);
    }

    pub fn get_all(&self) -> Vec<LogEntry> {
        self.logs.read().iter().cloned().collect()
    }

    pub fn get_recent(&self, count: usize) -> Vec<LogEntry> {
        let logs = self.logs.read();
        let start = if logs.len() > count { logs.len() - count } else { 0 };
        logs.iter().skip(start).cloned().collect()
    }

    pub fn clear(&self) {
        self.logs.write().clear();
    }
}

static LOG_BUFFER: std::sync::OnceLock<Arc<LogBuffer>> = std::sync::OnceLock::new();

pub fn get_log_buffer() -> Arc<LogBuffer> {
    LOG_BUFFER.get_or_init(|| Arc::new(LogBuffer::new())).clone()
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            println!("{}", msg);
            $crate::log::get_log_buffer().push("INFO", &msg);
        }
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            println!("[警告] {}", msg);
            $crate::log::get_log_buffer().push("WARN", &msg);
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            eprintln!("[错误] {}", msg);
            $crate::log::get_log_buffer().push("ERROR", &msg);
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            $crate::log::get_log_buffer().push("DEBUG", &msg);
        }
    };
}

pub fn push_log(level: &str, message: &str) {
    get_log_buffer().push(level, message);
}