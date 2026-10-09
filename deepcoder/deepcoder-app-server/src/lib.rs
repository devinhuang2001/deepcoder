//! DeepCoder AppServer — JSON-RPC 2.0 应用服务器
//!
//! 支持 Stdio、WebSocket、以及进程内 channel 传输。

pub mod protocol;
pub mod transport;

use anyhow::Result;

/// 启动 AppServer
pub async fn run(config: deepcoder_config::Config, addr: &str) -> Result<()> {
    tracing::info!("Starting AppServer WebSocket on {addr}");
    transport::run_ws_server(config, addr).await
}

/// 启动 stdio AppServer
pub async fn run_stdio(config: deepcoder_config::Config) -> Result<()> {
    transport::run_stdio(config).await
}
