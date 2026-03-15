use std::env;
use std::net::SocketAddr;

use crate::core::ServiceConfig;

pub struct Args;

impl Args {
    pub fn parse_to_config() -> Option<ServiceConfig> {
        let args: Vec<String> = env::args().collect();
        
        if args.len() == 1 {
            return None;
        }
        
        if args.iter().any(|a| a == "-h" || a == "--help") {
            print_help();
            std::process::exit(0);
        }
        
        Some(Self::parse_from_iter(args.iter().cloned()))
    }
    
    pub fn parse_line_to_config(line: &str) -> Option<ServiceConfig> {
        let args: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
        
        if args.is_empty() {
            return None;
        }
        
        Some(Self::parse_from_iter(args.into_iter()))
    }
    
    fn parse_from_iter<I: Iterator<Item = String>>(iter: I) -> ServiceConfig {
        let parsed = Self::parse_args_to_vec(iter);
        let mut config = ServiceConfig::default();
        
        for (k, v_opt) in parsed {
            match k.as_str() {
                "addr" | "a" => {
                    if let Some(v) = v_opt
                        && let Ok(addr) = v.parse::<SocketAddr>()
                    {
                        config.listen_addr = addr;
                    }
                }
                "api" => {
                    if let Some(v) = v_opt
                        && let Ok(addr) = v.parse::<SocketAddr>()
                    {
                        config.api_addr = addr;
                    }
                }
                "colo" | "c" => {
                    config.colo = v_opt.map(|v| {
                        v.split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    });
                }
                "dl" | "d" | "delay" => {
                    config.delay_limit = v_opt
                        .and_then(|v| v.parse::<u64>().ok())
                        .map_or(config.delay_limit, |v| v.clamp(1, 2000));
                }
                "tlr" | "l" | "loss" => {
                    config.tlr = v_opt
                        .and_then(|v| v.parse::<f64>().ok())
                        .map_or(config.tlr, |v| v.clamp(0.0, 1.0));
                }
                "http" | "u" | "url" => {
                    if let Some(v) = v_opt {
                        config.http = v;
                    }
                }
                "ips" | "i" => {
                    config.ips = v_opt
                        .and_then(|v| v.parse::<usize>().ok())
                        .map_or(config.ips, |v| v.clamp(1, 128));
                }
                "n" | "t" | "threads" => {
                    config.threads = v_opt
                        .and_then(|v| v.parse::<usize>().ok())
                        .map_or(config.threads, |v| v.clamp(1, 1024));
                }
                "tp" | "p" | "tls-port" => {
                    config.tls_port = v_opt
                        .and_then(|v| v.parse::<u16>().ok())
                        .map_or(config.tls_port, |v| v.clamp(1, u16::MAX));
                }
                "P" | "http-port" => {
                    config.http_port = v_opt
                        .and_then(|v| v.parse::<u16>().ok())
                        .map_or(config.http_port, |v| v.clamp(1, u16::MAX));
                }
                "f" | "file" => {
                    if let Some(v) = v_opt {
                        config.ip_file = v;
                    }
                }
                _ => {
                    print_help();
                    eprintln!("无效的参数: {k}");
                    std::process::exit(1);
                }
            }
        }
        
        config
    }

    fn parse_args_to_vec<I: Iterator<Item = String>>(iter: I) -> Vec<(String, Option<String>)> {
        let mut iter = iter.skip(1).peekable();
        let mut result = Vec::new();

        while let Some(arg) = iter.next() {
            if arg.starts_with('-') {
                let key = arg.trim_start_matches('-').to_string();
                let value = iter
                    .peek()
                    .filter(|next| !next.starts_with('-'))
                    .map(|next| next.to_string());

                if value.is_some() {
                    iter.next();
                }

                result.push((key, value));
            }
        }

        result
    }
}

fn approximate_display_width(s: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
            continue;
        } else if in_escape {
            if c == 'm' || c.is_alphabetic() {
                in_escape = false;
            }
            continue;
        }
        width += if c.is_ascii() { 1 } else { 2 };
    }
    width
}

fn format_help_line(name: &str, desc: &str, default: &str) -> String {
    let name_colored = format!("\x1b[32m{}\x1b[0m", name);
    let name_width = approximate_display_width(&name_colored);
    let name_padding = " ".repeat(11usize.saturating_sub(name_width));

    let desc_width = approximate_display_width(desc);
    let desc_padding = " ".repeat(45usize.saturating_sub(desc_width));

    let default_colored = format!("\x1b[2m{}\x1b[0m", default);
    let default_width = approximate_display_width(&default_colored);
    let default_padding = " ".repeat(15usize.saturating_sub(default_width));

    format!(
        " {}{}{}{}{}{}\n",
        name_colored,
        name_padding,
        desc,
        desc_padding,
        default_colored,
        default_padding
    )
}

pub fn print_help() {
    const HELP_ARGS: &[(&str, &str, &str)] = &[
        ("-addr", "本地监听的 IP 和端口", "127.6.6.6:6"),
        ("-api", "API 服务地址和端口，端口 0 自动分配", "127.0.0.1:0"),
        ("-colo", "筛选一个或多个数据中心，例如 HKG,LAX", "未指定"),
        ("-dl", "有效连接的平均延迟上限（毫秒）", "500"),
        ("-tlr", "有效连接的平均丢包率上限", "0.1"),
        ("-http", "测速地址", "http://cp.cloudflare.com/cdn-cgi/trace"),
        ("-ips", "目标负载 IP 数量", "10"),
        ("-n", "延迟测速并发上限", "16"),
        ("-tp", "TLS 流量使用的端口号", "443"),
        ("-p", "HTTP 流量使用的端口号", "80"),
        ("-f", "从文件读取 IP 或 CIDR", "ip.txt"),
    ];

    println!("\x1b[1;35m参数说明\x1b[0m\n");

    for (name, desc, default) in HELP_ARGS.iter() {
        print!("{}", format_help_line(name, desc, default));
    }
}