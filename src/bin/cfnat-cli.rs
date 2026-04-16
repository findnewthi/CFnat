use std::sync::Arc;

use cfnat::{args::Args, core::ServiceState};

#[tokio::main]
async fn main() {
    let service = Arc::new(ServiceState::new());

    if let Some(config) = Args::parse_to_config() {
        service.update_config(config);
    }

    if let Err(e) = service.start() {
        eprintln!("启动失败: {}", e);
        std::process::exit(1);
    }

    println!("CFnat CLI 已启动，按 Ctrl+C 退出。");

    tokio::signal::ctrl_c().await.ok();

    if let Err(e) = service.stop() {
        eprintln!("停止失败: {}", e);
        std::process::exit(1);
    }
}
