use std::sync::Arc;
use std::io::{self, BufRead};

use cfnat::{
    api::{create_router, AppState},
    args::Args,
    core::ServiceState,
};

fn print_banner() {
    println!(r#"
    ▄▄▄▄   ▄▄▄▄▄▄▄▄
  ██▀▀▀▀█  ██▀▀▀▀▀▀                        ██
 ██▀       ██        ██▄████▄   ▄█████▄  ███████
 ██        ███████   ██▀   ██   ▀ ▄▄▄██    ██
 ██▄       ██        ██    ██  ▄██▀▀▀██    ██
  ██▄▄▄▄█  ██        ██    ██  ██▄▄▄███    ██▄▄▄
    ▀▀▀▀   ▀▀        ▀▀    ▀▀   ▀▀▀▀ ▀▀     ▀▀▀▀
"#);
}

#[tokio::main]
async fn main() {
    print_banner();
    
    let service = Arc::new(ServiceState::new());
    
    if let Some(config) = Args::parse_to_config() {
        service.update_config(config);
    }
    let api_addr = service.get_config().api_addr;

    let api_state = AppState {
        service: service.clone(),
    };

    let app = create_router(api_state);
    
    let listener = match tokio::net::TcpListener::bind(api_addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("无法绑定 API 地址 {}: {}", api_addr, e);
            std::process::exit(1);
        }
    };
    
    let actual_addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    println!();
    println!("使用方式:");
    println!("  1. 按 Enter 使用默认配置启动");
    println!("  2. 输入参数启动");
    println!("  3. 通过 Web 界面控制: http://{}", actual_addr);
    println!();
    print!("> ");
    use std::io::Write;
    io::stdout().flush().ok();
    
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    
    let should_start = if let Some(Ok(line)) = lines.next() {
        if line.trim().is_empty() {
            println!("使用默认配置启动服务...");
            true
        } else if let Some(config) = Args::parse_line_to_config(&line) {
            service.update_config(config);
            true
        } else {
            println!("等待 Web 界面控制...");
            false
        }
    } else {
        false
    };
    
    if should_start {
        match service.start() {
            Ok(_) => println!("服务已启动"),
            Err(e) => {
                eprintln!("启动失败: {}", e);
                std::process::exit(1);
            }
        }
    }
    
    println!("按 Ctrl+C 退出...");
    tokio::signal::ctrl_c().await.ok();
}