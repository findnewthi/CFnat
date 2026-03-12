use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io;
use tokio::net::{TcpListener, TcpStream};

use crate::loadbalancer::LoadBalancer;

const READABLE_TIMEOUT_SECS: u64 = 10;

fn is_tls(buf: &[u8]) -> bool {
    buf.len() >= 1 && buf[0] == 0x16
}

async fn handle_client(
    client: TcpStream,
    lb: Arc<LoadBalancer>,
    tls_port: u16,
    http_port: u16,
) -> io::Result<()> {
    let mut buf = [0u8; 1];
    
    let peer_addr = client.peer_addr().ok();
    let (client_ip, source_port) = peer_addr.map(|a| (a.ip(), a.port())).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "无法获取客户端地址")
    })?;
    let client: TcpStream = client;
    client.peek(&mut buf).await?;

    let backend = lb.select(client_ip, source_port).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "无可用后端")
    })?;

    let sample_count = backend.get_sample_count();
    let avg_delay = backend.get_avg_delay();
    let loss_rate = backend.get_loss_rate() * 100.0;
    
    println!(
        "[选择] {} ← 负载均衡选择后端 (延迟：{:.1}ms, 丢包：{:.1}%, 样本：{})",
        backend.addr, avg_delay, loss_rate, sample_count
    );

    let (port, proto) = if is_tls(&buf) {
        (tls_port, "TLS")
    } else {
        (http_port, "HTTP")
    };

    let target_addr = std::net::SocketAddr::new(backend.addr.ip(), port);

    #[cfg(debug_assertions)]
    eprintln!(
        "[DEBUG] {} -> {} ({}) ← 用户发起请求",
        peer_addr.map(|a| a.to_string()).unwrap_or_default(),
        target_addr,
        proto
    );

    let result = async {
        let server = TcpStream::connect(target_addr).await?;
        
        let start = Instant::now();
        
        server.set_nodelay(true)?;
        let client = client;
        client.set_nodelay(true)?;
        
        let (mut client_read, mut client_write) = client.into_split();
        let (mut server_read, mut server_write) = server.into_split();
        
        let lb_inner = lb.clone();
        let backend_inner = backend.clone();
        
        let s2c = async move {
            match tokio::time::timeout(
                Duration::from_secs(READABLE_TIMEOUT_SECS),
                server_read.readable()
            ).await {
                Ok(Ok(_)) => {
                    let delay = start.elapsed().as_secs_f32() * 1000.0;
                    lb_inner.record_delay(&backend_inner, delay);
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("后端 {} 秒无响应", READABLE_TIMEOUT_SECS)
                    ));
                }
            }
            
            io::copy(&mut server_read, &mut client_write).await
        };
        
        let c2s = io::copy(&mut client_read, &mut server_write);
        
        let result = tokio::select! {
            res = c2s => res,
            res = s2c => res,
        };
        
        result
    }
    .await;
    
    if result.is_err() {
        lb.record_loss(&backend, true);
    } else {
        lb.record_loss(&backend, false);
    }

    let should_evict = lb.check_and_evict(&backend);
    
    if should_evict {
        let avg_delay = backend.get_avg_delay();
        let loss_rate = backend.get_loss_rate();
        let delay_threshold = lb.get_delay_threshold();
        lb.remove_backend(backend.clone());
        lb.refill_from_backup();
        println!("[剔除] {} 延迟 {:.1}ms/阈值 {:.1}ms 丢包率 {:.1}% ← 用户请求触发", 
            backend.addr, avg_delay, delay_threshold, loss_rate * 100.0);
    }

    lb.release(&backend);
    result?;

    Ok(())
}

pub(crate) async fn run_forward(
    listen_addr: SocketAddr,
    lb: Arc<LoadBalancer>,
    tls_port: u16,
    http_port: u16,
) -> io::Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;

    println!("监听 {}", listen_addr);
    println!("TLS 端口: {}, HTTP 端口: {}", tls_port, http_port);
    println!("负载均衡 IP 数量: {}", lb.get_primary_count());
    println!("备选 IP 数量: {}", lb.get_backup_count());

    loop {
        let (client, _) = listener.accept().await?;

        let lb = lb.clone();
        tokio::spawn(async move {
            if let Err(_e) = handle_client(client, lb, tls_port, http_port).await {}
        });
    }
}