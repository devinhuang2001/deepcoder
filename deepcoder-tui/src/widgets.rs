//! TUI Widget 组件

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::layout::{Alignment, Rect};
use ratatui::Frame;

/// 渲染聊天面板
pub fn render_chat(frame: &mut Frame, area: Rect, messages: &[String], streaming: bool) {
    let text: Vec<Line> = messages.iter()
        .flat_map(|msg| {
            if msg.starts_with('>') {
                vec![
                    Line::from(Span::styled(msg, Style::default().fg(Color::Cyan))),
                    Line::from(""),
                ]
            } else {
                vec![Line::from(msg.as_str()), Line::from("")]
            }
        })
        .collect();

    let chat = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Chat"))
        .wrap(Wrap { trim: false });

    frame.render_widget(chat, area);
}

/// 渲染推理面板
pub fn render_reasoning(frame: &mut Frame, area: Rect, content: &str, visible: bool) {
    if !visible || content.is_empty() {
        return;
    }

    let text = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Thinking"))
        .style(Style::default().fg(Color::DarkGray).italic())
        .wrap(Wrap { trim: false });

    frame.render_widget(text, area);
}

/// 渲染输入区域
pub fn render_input(frame: &mut Frame, area: Rect, input: &str, streaming: bool) {
    let prefix = if streaming { "▶ " } else { "> " };
    let text = Paragraph::new(format!("{prefix}{input}"))
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .style(if streaming {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        });

    frame.render_widget(text, area);
}

/// 渲染状态行
pub fn render_status(frame: &mut Frame, area: Rect, message: &str) {
    let text = Paragraph::new(message)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    frame.render_widget(text, area);
}
