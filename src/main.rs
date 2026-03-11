mod args;
mod forward;
mod hyper;
mod httping;
mod ip;
mod loadbalancer;
mod pool;

use std::sync::Arc;

use crate::args::{Args, print_help};
use crate::forward::run_forward;
use crate::httping::run_continuous_httping;
use crate::ip::IpPool;
use crate::loadbalancer::LoadBalancer;
use crate::pool::init_global_limiter;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.help {
        print_help();
        return;
    }

    let ip_pool = Arc::new(IpPool::from_file(&args.ip_file));
    if ip_pool.total_count() == 0 {
        eprintln!("未找到有效的 IP");
        std::process::exit(1);
    }

    println!("解析到 {} 个 IP", ip_pool.total_count());

    init_global_limiter(args.threads);

    let (notify_tx, notify_rx) = tokio::sync::watch::channel(false);

    let lb = Arc::new(
        LoadBalancer::new(args.ips as usize)
            .with_delay_limit(args.delay_limit as f32)
            .with_delay_threshold(args.delay_limit as f32)
            .with_loss_threshold(args.tlr as f32)
            .with_health_check_url(args.http.clone())
            .with_ports(args.tls_port, args.http_port)
            .with_timeout(args.delay_limit)
            .with_notify(notify_tx),
    );

    let colo_filter: Option<Vec<String>> = args.colo.clone();

    let ip_pool_clone = ip_pool.clone();
    let lb_clone = lb.clone();
    let http = args.http.clone();
    
    tokio::spawn(async move {
        run_continuous_httping(
            ip_pool_clone,
            lb_clone,
            &http,
            args.tls_port,
            args.http_port,
            args.delay_limit,
            args.delay_limit,
            colo_filter.as_deref(),
            notify_rx,
        )
        .await;
    });

    lb.clone().start_health_check();

    println!("等待主队列填充...");
    while lb.get_primary_count() == 0 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    println!(
        "主队列: {}/{}，备选: {}/{}",
        lb.get_primary_count(),
        lb.get_primary_target(),
        lb.get_backup_count(),
        lb.get_backup_target()
    );

    if let Err(e) = run_forward(args.addr, lb, args.tls_port, args.http_port).await {
        eprintln!("转发错误: {}", e);
    }
}