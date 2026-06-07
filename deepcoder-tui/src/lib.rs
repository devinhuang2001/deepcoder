//! DeepCoder TUI — ratatui 驱动的终端界面

pub mod app;
pub mod widgets;
pub mod render;

use anyhow::Result;

/// 启动 TUI
pub async fn run(config: deepcoder_config::Config) -> Result<()> {
    tracing::info!("TUI starting");
    let _ = config;
    tracing::info!("TUI mode ready (interactive mode pending)");
    Ok(())
}
