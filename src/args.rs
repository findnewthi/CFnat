use std::env;
use std::net::SocketAddr;

#[derive(Clone)]
pub(crate) struct Args {
    pub(crate) addr: SocketAddr,
    pub(crate) colo: Option<Vec<String>>,
    pub(crate) delay_limit: u64,
    pub(crate) tlr: f32,
    pub(crate) http: String,
    pub(crate) ips: usize,
    pub(crate) threads: usize,
    pub(crate) tls_port: u16,
    pub(crate) http_port: u16,
    pub(crate) ip_file: String,
    pub(crate) help: bool,
}

impl Args {
    pub(crate) fn new() -> Self {
        Self {
            addr: "127.6.6.6:6".parse().unwrap(),
            colo: None,
            delay_limit: 500,
            tlr: 0.1,
            http: "http://cp.cloudflare.com/cdn-cgi/trace".to_string(),
            ips: 10,
            threads: 16,
            tls_port: 443,
            http_port: 80,
            ip_file: "ip.txt".to_string(),
            help: false,
        }
    }

    pub(crate) fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut parsed = Self::new();
        let vec = Self::parse_args_to_vec(&args);

        for (k, v_opt) in vec {
            match k.as_str() {
                "h" | "help" => parsed.help = true,
                "addr" => {
                    if let Some(v) = v_opt {
                        if let Ok(addr) = v.parse() {
                            parsed.addr = addr;
                        } else {
                            eprintln!("无效的监听地址: {}", v);
                            std::process::exit(1);
                        }
                    }
                }
                "colo" => {
                    parsed.colo = v_opt.map(|v| {
                        v.split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    });
                }
                "dl" => {
                    parsed.delay_limit = v_opt
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(parsed.delay_limit)
                        .clamp(1, 10000);
                }
                "tlr" => {
                    parsed.tlr = v_opt
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(parsed.tlr)
                        .clamp(0.0, 1.0);
                }
                "http" => {
                    if let Some(v) = v_opt {
                        parsed.http = v;
                    }
                }
                "ips" => {
                    parsed.ips = v_opt
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(parsed.ips)
                        .clamp(1, 1000);
                }
                "n" => {
                    parsed.threads = v_opt
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(parsed.threads)
                        .clamp(1, 1024);
                }
                "tp" => {
                    parsed.tls_port = v_opt
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(parsed.tls_port)
                        .clamp(1, u16::MAX);
                }
                "p" => {
                    parsed.http_port = v_opt
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(parsed.http_port)
                        .clamp(1, u16::MAX);
                }
                "f" => {
                    if let Some(v) = v_opt {
                        parsed.ip_file = v;
                    }
                }
                _ => {
                    print_help();
                    eprintln!("无效的参数: {k}");
                    std::process::exit(1);
                }
            }
        }

        parsed
    }

    fn parse_args_to_vec(args: &[String]) -> Vec<(String, Option<String>)> {
        let mut iter = args.iter().skip(1).peekable();
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

pub(crate) fn print_help() {
    const HELP_ARGS: &[(&str, &str, &str)] = &[
        ("-addr", "本地监听的 IP 和端口", "127.6.6.6:6"),
        ("-colo", "筛选一个或多个数据中心，例如 HKG,LAX", "未指定"),
        ("-dl", "有效连接的平均延迟上限（毫秒）", "500"),
        ("-tlr", "有效连接的平均丢包率上限", "0.1"),
        ("-http", "测速地址", "http://cp.cloudflare.com/cdn-cgi/trace"),
        ("-ips", "目标负载 IP 数量", "10"),
        ("-n", "延迟测速并发上限", "16"),
        ("-tp", "TLS 流量使用的端口号", "443"),
        ("-p", "HTTP 流量使用的端口号", "80"),
        ("-f", "从文件读取 IP 或 CIDR", "未指定"),
    ];

    println!("\x1b[1;35m参数说明\x1b[0m\n");

    for (name, desc, default) in HELP_ARGS.iter() {
        print!("{}", format_help_line(name, desc, default));
    }
}