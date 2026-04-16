use std::io::{self, BufRead, Write};
use std::sync::Arc;

use cfnat::{args::Args, core::ServiceState};

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

async fn run(service: Arc<ServiceState>) {
    match service.start() {
        Ok(_) => println!("服务已启动"),
        Err(e) => {
            eprintln!("启动失败: {}", e);
            std::process::exit(1);
        }
    }

    tokio::signal::ctrl_c().await.ok();

    match service.stop() {
        Ok(_) => println!("服务已停止"),
        Err(e) => {
            eprintln!("停止失败: {}", e);
            std::process::exit(1);
        }
    }
}

#[tokio::main]
async fn main() {
    print_banner();

    let service = Arc::new(ServiceState::new());

    if let Some(config) = Args::parse_to_config() {
        service.update_config(config);
        run(service).await;
        return;
    }

    run(service).await;
}
