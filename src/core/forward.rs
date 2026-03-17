use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::core::loadbalancer::LoadBalancer;

const READABLE_TIMEOUT_SECS: u64 = 10;

fn is_tls(buf: &[u8]) -> bool {
    !buf.is_empty() && buf[0] == 0x16
}

async fn handle_client(
    client: TcpStream,
    lb: Arc<LoadBalancer>,
    tls_port: u16,
    http_port: u16,
) -> io::Result<()> {
    let mut buf = [0u8; 1];
    
    client.peek(&mut buf).await?;

    let backend = lb.select().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "无可用后端")
    })?;

    let port = if is_tls(&buf) {
        tls_port
    } else {
        http_port
    };

    let target_addr = std::net::SocketAddr::new(backend.addr.ip(), port);

    let result = async {
        let server = TcpStream::connect(target_addr).await?;
        
        let start = Instant::now();
        
        server.set_nodelay(true)?;
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
        let delay_threshold = lb.get_delay_threshold();
        lb.remove_backend(backend.clone());
        lb.refill_from_backup();
        println!("[-] {} 延迟{:.0}ms>{:.0}ms (请求触发)", backend.addr, avg_delay, delay_threshold);
    }

    lb.release(&backend);
    result?;

    Ok(())
}

pub async fn run_forward(
    listen_addr: SocketAddr,
    lb: Arc<LoadBalancer>,
    tls_port: u16,
    http_port: u16,
    cancel_token: CancellationToken,
) -> io::Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;

    println!("转发服务 {} (TLS:{}, HTTP:{})", 
        listen_addr, tls_port, http_port);

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                let (client, _) = accept_result?;
                let lb = lb.clone();
                tokio::spawn(async move {
                    if let Err(_e) = handle_client(client, lb, tls_port, http_port).await {}
                });
            }
            _ = cancel_token.cancelled() => {
                println!("[转发服务] 收到停止信号，退出");
                break;
            }
        }
    }

    Ok(())
}