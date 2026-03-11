use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::loadbalancer::LoadBalancer;

fn is_tls(buf: &[u8]) -> bool {
    buf.len() >= 1 && buf[0] == 0x16
}

fn is_http(buf: &[u8]) -> bool {
    buf.len() >= 1 && buf[0] != 0x16
}

async fn handle_client(
    mut client: TcpStream,
    lb: &LoadBalancer,
    tls_port: u16,
    http_port: u16,
) -> io::Result<()> {
    let mut buf = [0u8; 8192];
    let n = client.read(&mut buf).await?;

    if n == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "无数据"));
    }

    let backend = lb.select().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "无可用后端")
    })?;

    let sample_count = backend.get_sample_count();
    let avg_delay = backend.get_avg_delay();
    let loss_rate = backend.get_loss_rate() * 100.0;
    
    println!(
        "[选择] {} ← 负载均衡选择后端 (延迟: {:.1}ms, 丢包: {:.1}%, 样本: {})",
        backend.addr, avg_delay, loss_rate, sample_count
    );

    let (port, proto) = if is_tls(&buf[..n]) {
        (tls_port, "TLS")
    } else if is_http(&buf[..n]) {
        (http_port, "HTTP")
    } else {
        (tls_port, "未知")
    };

    let target_addr = std::net::SocketAddr::new(backend.addr.ip(), port);

    #[cfg(debug_assertions)]
    eprintln!(
        "[DEBUG] {} -> {} ({}) ← 用户发起请求",
        client
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_default(),
        target_addr,
        proto
    );

    let start = Instant::now();
    let mut connect_delay = 0.0f32;

    let result = async {
        let mut server = TcpStream::connect(target_addr).await?;
        connect_delay = start.elapsed().as_secs_f32() * 1000.0;
        
        lb.record_delay(&backend, connect_delay);
        lb.record_loss(&backend, false);
        
        server.write_all(&buf[..n]).await?;
        io::copy_bidirectional(&mut client, &mut server).await?;
        Ok::<_, io::Error>(())
    }
    .await;
    
    if result.is_err() {
        lb.record_loss(&backend, true);
    }

    let should_evict = lb.check_and_evict(&backend);
    
    if should_evict {
        lb.remove_backend(backend.clone());
        lb.refill_from_backup();
        println!("[剔除] {} 延迟 {:.1}ms 超阈值 ← 用户请求触发", backend.addr, connect_delay);
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

    lb.clone().start_health_check();

    loop {
        let (client, _) = listener.accept().await?;

        let lb = lb.clone();
        tokio::spawn(async move {
            if let Err(_e) = handle_client(client, &lb, tls_port, http_port).await {}
        });
    }
}