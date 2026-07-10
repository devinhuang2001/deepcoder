//! TUI Widget 组件

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

/// 渲染聊天面板
pub fn render_chat(frame: &mut Frame, area: Rect, messages: &[String], _streaming: bool) {
    let text: Vec<Line> = messages
        .iter()
        .flat_map(|msg| {
            let mut lines = format_message_lines(msg);
            lines.push(Line::from(""));
            lines
        })
        .collect();

    let chat = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Chat"))
        .wrap(Wrap { trim: false });

    frame.render_widget(chat, area);
}

pub fn format_message_lines(message: &str) -> Vec<Line<'static>> {
    if message.starts_with('>') {
        return vec![Line::from(Span::styled(
            message.to_string(),
            Style::default().fg(Color::Cyan),
        ))];
    }

    let mut in_code = false;
    message
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                in_code = !in_code;
                return Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Magenta),
                ));
            }
            if line.starts_with('+') {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Green),
                ))
            } else if line.starts_with('-') {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Red),
                ))
            } else if line.starts_with('#') {
                Line::from(Span::styled(
                    line.trim_start_matches('#').trim().to_string(),
                    Style::default().fg(Color::Yellow).bold(),
                ))
            } else if in_code {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::LightBlue),
                ))
            } else {
                Line::from(line.to_string())
            }
        })
        .collect()
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

pub fn render_approval_overlay(
    frame: &mut Frame,
    area: Rect,
    tool_name: &str,
    message: &str,
    command: Option<&str>,
) {
    let width = area.width.saturating_sub(4).min(76);
    let height = if command.is_some() { 9 } else { 7 };
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);
    let mut lines = vec![
        Line::from(Span::styled(
            format!("Approve tool: {tool_name}"),
            Style::default().fg(Color::Yellow).bold(),
        )),
        Line::from(""),
        Line::from(message.to_string()),
    ];
    if let Some(command) = command {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            command.to_string(),
            Style::default().fg(Color::LightBlue),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Enter/y approve   n/Esc deny",
        Style::default().fg(Color::Green),
    )));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Approval"))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

pub fn render_session_picker(
    frame: &mut Frame,
    area: Rect,
    sessions: &[deepcoder_persistence::index::SessionEntry],
    selected: usize,
) {
    let width = area.width.saturating_sub(4).min(88);
    let height = (sessions.len() as u16 + 4).min(area.height.saturating_sub(2).max(6));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);
    let mut lines = Vec::new();
    for (index, session) in sessions.iter().enumerate() {
        let marker = if index == selected { "> " } else { "  " };
        let summary = session
            .summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&session.model);
        let line = format!(
            "{marker}{}  messages:{}  {}",
            short_id(&session.id),
            session.message_count,
            summary
        );
        let style = if index == selected {
            Style::default().fg(Color::Yellow).bold()
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(line, style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Enter resume   f fork   Esc close",
        Style::default().fg(Color::Green),
    )));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Sessions"))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn format_message_lines_styles_markdown_and_diff() {
        let lines = format_message_lines("# Title\n```rust\nfn main() {}\n```\n+add\n-remove");
        assert_eq!(lines[0].spans[0].content, "Title");
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Yellow));
        assert_eq!(lines[2].spans[0].style.fg, Some(Color::LightBlue));
        assert_eq!(lines[4].spans[0].style.fg, Some(Color::Green));
        assert_eq!(lines[5].spans[0].style.fg, Some(Color::Red));
    }

    #[test]
    fn widgets_render_without_panic() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_chat(
                    frame,
                    Rect::new(0, 0, 80, 10),
                    &["> hello".into(), "# Answer\n+ok".into()],
                    false,
                );
                render_reasoning(frame, Rect::new(0, 10, 80, 4), "thinking", true);
                render_input(frame, Rect::new(0, 14, 80, 3), "input", false);
                render_status(frame, Rect::new(0, 17, 80, 3), "Ready");
                render_approval_overlay(
                    frame,
                    Rect::new(0, 0, 80, 20),
                    "bash",
                    "command requires approval",
                    Some("Get-Date"),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Chat"));
        assert!(rendered.contains("Answer"));
        assert!(rendered.contains("Ready"));
        assert!(rendered.contains("Approval"));
        assert!(rendered.contains("Get-Date"));
    }

    #[test]
    fn session_picker_renders_sessions() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_session_picker(
                    frame,
                    Rect::new(0, 0, 80, 20),
                    &[deepcoder_persistence::index::SessionEntry {
                        id: "12345678-session".into(),
                        thread_id: "thread".into(),
                        model: "deepseek-chat".into(),
                        created_at: "2026-01-01T00:00:00Z".into(),
                        updated_at: "2026-01-01T00:00:00Z".into(),
                        message_count: 2,
                        summary: Some("summary".into()),
                    }],
                    0,
                );
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Sessions"));
        assert!(rendered.contains("summary"));
    }
}
