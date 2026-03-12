use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use http::Method;
use tokio::task::JoinSet;

use crate::hyper::{parse_url, send_request};
use crate::ip::IpPool;
use crate::loadbalancer::LoadBalancer;
use crate::pool::GLOBAL_LIMITER;

const PING_TIMES: u8 = 4;
const CACHE_MAX_SIZE: usize = 50;

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
) -> Option<(f32, Option<String>, u16)> {
    let start = Instant::now();

    let resp = send_request(client, host, uri.clone(), Method::HEAD, timeout_ms).await?;

    let delay = start.elapsed().as_secs_f32() * 1000.0;

    let colo = extract_colo(&resp);
    let status = resp.status().as_u16();

    Some((delay, colo, status))
}

pub(crate) async fn http_ping_multi(
    ip: std::net::IpAddr,
    tls_port: u16,
    http_port: u16,
    client: Arc<crate::hyper::MyHyperClient>,
    host: Arc<str>,
    scheme: Arc<str>,
    path: Arc<str>,
    timeout_ms: u64,
    colo_filter: Option<Arc<Vec<String>>>,
) -> Option<(SocketAddr, f32, Option<String>, u8)> {
    let _permit = GLOBAL_LIMITER.get()?.acquire().await;

    let port = if &*scheme == "https" { tls_port } else { http_port };
    let addr = SocketAddr::new(ip, port);
    let uri: http::Uri = format!("{}://{}{}", &*scheme, addr, &*path).parse().ok()?;

    let mut total_delay = 0.0f32;
    let mut success_count = 0u8;
    let mut colo: Option<String> = None;
    let mut last_status: u16 = 0;
    let mut should_stop = false;

    for _ in 0..PING_TIMES {
        if should_stop {
            break;
        }

        if let Some((delay, c, status)) = single_ping(&client, &host, &uri, timeout_ms).await {
            total_delay += delay;
            success_count += 1;
            last_status = status;

            if colo.is_none() {
                if let Some(ref filter) = colo_filter {
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

    #[cfg(debug_assertions)]
    {
        let colo_str = colo.as_deref().unwrap_or("???");
        eprintln!(
            "[DEBUG] {} -> {:.1}ms avg ({}/{}) [{}] status={} ← 并发测速完成",
            addr, avg_delay, success_count, PING_TIMES, colo_str, last_status
        );
    }

    Some((addr, avg_delay, colo, success_count))
}

pub(crate) async fn run_continuous_httping(
    ip_pool: Arc<IpPool>,
    lb: Arc<LoadBalancer>,
    url: &str,
    tls_port: u16,
    http_port: u16,
    timeout_ms: u64,
    delay_limit: u64,
    colo_filter: Option<&[String]>,
    mut notify_rx: tokio::sync::watch::Receiver<bool>,
    client: Arc<crate::hyper::MyHyperClient>,
) {
    let (_, host, scheme, path) = match parse_url(url) {
        Some(r) => r,
        None => {
            eprintln!("URL 解析失败");
            return;
        }
    };

    let host: Arc<str> = Arc::from(host.as_str());
    let scheme: Arc<str> = Arc::from(scheme);
    let path: Arc<str> = Arc::from(path.as_str());
    let colo_filter: Option<Arc<Vec<String>>> = colo_filter.map(|v| Arc::new(v.to_vec()));

    println!("开始持续测速...");

    let concurrency = GLOBAL_LIMITER.get().map(|l| l.max_concurrent()).unwrap_or(128);
    let mut tasks: JoinSet<Option<(SocketAddr, f32, Option<String>, u8)>> = JoinSet::new();

    let spawn_task = |ip: std::net::IpAddr| {
        let client = client.clone();
        let host = host.clone();
        let scheme = scheme.clone();
        let path = path.clone();
        let colo_filter = colo_filter.clone();

        async move {
            http_ping_multi(
                ip,
                tls_port,
                http_port,
                client,
                host,
                scheme,
                path,
                timeout_ms,
                colo_filter,
            )
            .await
        }
    };

    let mut result_cache: VecDeque<(SocketAddr, f32, Option<String>, u8)> = VecDeque::new();

    let try_fill_from_cache = |cache: &mut VecDeque<(SocketAddr, f32, Option<String>, u8)>| {
        while let Some((addr, delay, colo, success_count)) = cache.pop_front() {
            if lb.contains(addr.ip()) {
                continue;
            }
            
            let primary_count = lb.get_primary_count();
            let backup_count = lb.get_backup_count();
            let primary_target = lb.get_primary_target();
            let backup_target = lb.get_backup_target();

            if primary_count < primary_target {
                lb.add_to_primary(addr);
                println!(
                    "[主队列] {} -> {:.1}ms ({}/{}) [{}] ← 缓存填充（测速结果）",
                    addr,
                    delay,
                    success_count,
                    PING_TIMES,
                    colo.as_deref().unwrap_or("???")
                );
            } else if backup_count < backup_target {
                lb.add_to_backup(addr);
                println!(
                    "[备选] {} -> {:.1}ms ({}/{}) [{}] ← 缓存填充（测速结果）",
                    addr,
                    delay,
                    success_count,
                    PING_TIMES,
                    colo.as_deref().unwrap_or("???")
                );
            } else {
                cache.push_front((addr, delay, colo, success_count));
                return;
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

            tasks.spawn(spawn_task(ip));
        }

        match tasks.join_next().await {
            Some(Ok(result)) => {
                if let Some((addr, delay, colo, success_count)) = result {
                    if delay <= delay_limit as f32 && !lb.contains(addr.ip()) {
                        let primary_count = lb.get_primary_count();
                        let backup_count = lb.get_backup_count();
                        let primary_target = lb.get_primary_target();
                        let backup_target = lb.get_backup_target();

                        if primary_count < primary_target {
                            lb.add_to_primary(addr);
                            println!(
                                "[主队列] {} -> {:.1}ms ({}/{}) [{}] ← 新测速结果（队列未满）",
                                addr,
                                delay,
                                success_count,
                                PING_TIMES,
                                colo.as_deref().unwrap_or("???")
                            );
                        } else if backup_count < backup_target {
                            lb.add_to_backup(addr);
                            println!(
                                "[备选] {} -> {:.1}ms ({}/{}) [{}] ← 新测速结果（队列未满）",
                                addr,
                                delay,
                                success_count,
                                PING_TIMES,
                                colo.as_deref().unwrap_or("???")
                            );
                        } else if result_cache.len() < CACHE_MAX_SIZE {
                            result_cache.push_back((addr, delay, colo, success_count));
                        }
                    }
                }
            }
            Some(Err(_)) | None => {}
        }
    }
}