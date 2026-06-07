//! DeepCoder AppServer — JSON-RPC 2.0 应用服务器
//!
//! 支持 Stdio、WebSocket、以及进程内 channel 传输。

pub mod protocol;

use anyhow::Result;

/// 启动 AppServer
pub async fn run(config: deepcoder_config::Config, addr: &str) -> Result<()> {
    tracing::info!("Starting AppServer on {addr}");
    // TODO: 实现 WebSocket 传输 + JSON-RPC 消息循环
    Ok(())
}
