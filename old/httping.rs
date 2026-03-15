use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use http::Method;
use tokio::task::JoinSet;

use crate::config::get_global_config;
use crate::hyper::{parse_url, send_request};
use crate::ip::IpPool;
use crate::loadbalancer::{AddResult, LoadBalancer};
use crate::pool::GLOBAL_LIMITER;

type PingResult = Option<(SocketAddr, f32, Option<String>, u8)>;

fn extract_colo(resp: &hyper::Response<hyper::body::Incoming>) -> Option<String> {
    resp.headers()
        .get("cf-ray")?
        .to_str()
        .ok()?
        .rsplit('-')
        .next()
        .map(str::to_owned)
}

async fn single_ping(
    client: &crate::hyper::MyHyperClient,
    host: &str,
    uri: &http::Uri,
    timeout_ms: u64,
) -> Option<(f32, Option<String>)> {
    let start = Instant::now();

    let resp = send_request(client, host, uri.clone(), Method::HEAD, timeout_ms).await?;

    let delay = start.elapsed().as_secs_f32() * 1000.0;

    let colo = extract_colo(&resp);

    Some((delay, colo))
}

#[derive(Clone)]
pub(crate) struct PingConfig {
    pub(crate) tls_port: u16,
    pub(crate) http_port: u16,
    pub(crate) client: Arc<crate::hyper::MyHyperClient>,
    pub(crate) host: Arc<str>,
    pub(crate) scheme: Arc<str>,
    pub(crate) path: Arc<str>,
    pub(crate) timeout_ms: u64,
    pub(crate) colo_filter: Option<Arc<Vec<String>>>,
}

pub(crate) async fn http_ping_multi(
    ip: std::net::IpAddr,
    config: &PingConfig,
) -> Option<(SocketAddr, f32, Option<String>, u8)> {
    let _permit = GLOBAL_LIMITER.get()?.acquire().await;

    let port = if &*config.scheme == "https" { config.tls_port } else { config.http_port };
    let addr = SocketAddr::new(ip, port);
    let uri: http::Uri = format!("{}://{}{}", &*config.scheme, addr, &*config.path).parse().ok()?;

    let mut total_delay = 0.0f32;
    let mut success_count = 0u8;
    let mut colo: Option<String> = None;
    let mut should_stop = false;

    for _ in 0..get_global_config().ping_times {
        if should_stop {
            break;
        }

        if let Some((delay, c)) = single_ping(&config.client, &config.host, &uri, config.timeout_ms).await {
            total_delay += delay;
            success_count += 1;

            if colo.is_none() {
                if let Some(ref filter) = config.colo_filter {
                    if let Some(ref dc) = c {
                        if !filter.iter().any(|f| f.eq_ignore_ascii_case(dc)) {
                            should_stop = true;
                        }
                    } else {
                        should_stop = true;
                    }
                }
                colo = c;
            }

            if !should_stop {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }

    if success_count == 0 || should_stop {
        return None;
    }

    let avg_delay = total_delay / success_count as f32;

    Some((addr, avg_delay, colo, success_count))
}

pub(crate) struct HttpingConfig {
    pub(crate) tls_port: u16,
    pub(crate) http_port: u16,
    pub(crate) timeout_ms: u64,
    pub(crate) delay_limit: u64,
    pub(crate) colo_filter: Option<Arc<Vec<String>>>,
    pub(crate) client: Arc<crate::hyper::MyHyperClient>,
}

pub(crate) async fn run_continuous_httping(
    ip_pool: Arc<IpPool>,
    lb: Arc<LoadBalancer>,
    url: &str,
    config: HttpingConfig,
    mut notify_rx: tokio::sync::watch::Receiver<bool>,
) {
    let (_, host, scheme, path) = match parse_url(url) {
        Some(r) => r,
        None => {
            eprintln!("URL 解析失败");
            return;
        }
    };

    let ping_config = PingConfig {
        tls_port: config.tls_port,
        http_port: config.http_port,
        client: config.client,
        host: Arc::from(host.as_str()),
        scheme: Arc::from(scheme),
        path: Arc::from(path.as_str()),
        timeout_ms: config.timeout_ms,
        colo_filter: config.colo_filter,
    };

    println!("开始持续测速...");

    let concurrency = GLOBAL_LIMITER.get().unwrap().max_concurrent();
    let mut tasks: JoinSet<PingResult> = JoinSet::new();

    let spawn_task = |ip: std::net::IpAddr, cfg: &PingConfig| {
        let cfg = PingConfig {
            tls_port: cfg.tls_port,
            http_port: cfg.http_port,
            client: cfg.client.clone(),
            host: cfg.host.clone(),
            scheme: cfg.scheme.clone(),
            path: cfg.path.clone(),
            timeout_ms: cfg.timeout_ms,
            colo_filter: cfg.colo_filter.clone(),
        };

        async move { http_ping_multi(ip, &cfg).await }
    };

    let mut result_cache: VecDeque<(SocketAddr, f32, Option<String>, u8)> = VecDeque::new();

    let try_fill_from_cache = |cache: &mut VecDeque<(SocketAddr, f32, Option<String>, u8)>| {
        while let Some((addr, delay, colo, success_count)) = cache.pop_front() {
            let result = lb.try_add_backend(
                addr,
                delay,
                colo.as_deref(),
                success_count,
                "缓存填充（测速结果）",
            );
            
            match result {
                AddResult::AddedToPrimary | AddResult::AddedToBackup => {}
                AddResult::QueueFull | AddResult::AlreadyExists => {
                    cache.push_front((addr, delay, colo, success_count));
                    return;
                }
            }
        }
    };

    loop {
        try_fill_from_cache(&mut result_cache);

        if lb.should_pause() {
            let _ = notify_rx.changed().await;
            continue;
        }

        while tasks.len() < concurrency {
            let Some(ip) = ip_pool.pop() else {
                break;
            };

            if lb.contains(ip) {
                continue;
            }

            tasks.spawn(spawn_task(ip, &ping_config));
        }

        if let Some(Ok(result)) = tasks.join_next().await
            && let Some((addr, delay, colo, success_count)) = result
            && delay <= config.delay_limit as f32
        {
            let add_result = lb.try_add_backend(
                addr,
                delay,
                colo.as_deref(),
                success_count,
                "新测速结果（队列未满）",
            );

            if let AddResult::QueueFull = add_result
                && result_cache.len() < get_global_config().sample_window as usize
            {
                result_cache.push_back((addr, delay, colo, success_count));
            }
        }
    }
}