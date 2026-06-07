//! DeepCoder TUI — ratatui 驱动的终端界面

pub mod app;
pub mod widgets;
pub mod render;

use anyhow::Result;

/// 启动 TUI
pub async fn run(config: deepcoder_config::Config) -> Result<()> {
    let mut terminal = ratatui::init();
    let app = app::App::new(config);
    let result = app.run(&mut terminal).await;
    ratatui::restore();
    result
}
