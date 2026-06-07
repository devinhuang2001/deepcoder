//! DeepCoder MCP 集成
//!
//! 既是 MCP 客户端（消费外部工具），也是 MCP 服务器（暴露自身工具）。

use anyhow::Result;

/// 以 MCP 服务器模式运行
pub async fn run_server(config: deepcoder_config::Config) -> Result<()> {
    tracing::info!("Starting MCP server mode");
    let _ = config;
    // TODO: 实现 MCP 服务器 (stdio JSON-RPC)
    Ok(())
}
