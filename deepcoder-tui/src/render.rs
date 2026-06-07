//! TUI 主渲染循环

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

use crate::app::App;
use crate::widgets;

/// 主渲染函数
pub fn render(app: &mut App, frame: &mut Frame) {
    let area = frame.area();

    // 布局：上半部聊天 | 推理面板 | 下半部输入 + 状态
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),      // 聊天区域
            Constraint::Length(8),   // 推理面板
            Constraint::Length(3),   // 输入区域
            Constraint::Length(1),   // 状态行
        ])
        .split(area);

    // 聊天区域
    widgets::render_chat(frame, chunks[0], &app.messages, app.streaming);

    // 推理面板
    widgets::render_reasoning(frame, chunks[1], &app.reasoning, app.show_reasoning);

    // 输入区域
    widgets::render_input(frame, chunks[2], &app.input, app.streaming);

    // 状态行
    let status = if app.streaming { "🔄 Streaming..." } else { "✓ Ready" };
    widgets::render_status(frame, chunks[3], status);
}
