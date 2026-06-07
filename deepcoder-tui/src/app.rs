//! TUI 主应用

use ratatui::widgets::{Block, Borders};
use ratatui::layout::{Layout, Direction, Constraint};
use tokio::sync::broadcast;
use deepcoder_types::event::EngineEvent;

use crate::widgets;

/// TUI 应用状态
pub struct App {
    config: deepcoder_config::Config,
    session: deepcoder_engine::Session,
    event_rx: broadcast::Receiver<EngineEvent>,
    /// 聊天消息
    pub messages: Vec<String>,
    /// 输入缓冲区
    pub input: String,
    /// 推理内容
    pub reasoning: String,
    /// 是否正在流式响应
    pub streaming: bool,
    /// 是否显示推理面板
    pub show_reasoning: bool,
}

impl App {
    pub fn new(config: deepcoder_config::Config) -> Self {
        let tool_router = std::sync::Arc::new(deepcoder_tools::ToolRouter::new());
        let session = deepcoder_engine::Session::new(config.clone(), tool_router);
        let (tx, rx) = broadcast::channel(1024);

        Self {
            config,
            session,
            event_rx: rx,
            messages: Vec::new(),
            input: String::new(),
            reasoning: String::new(),
            streaming: false,
            show_reasoning: true,
        }
    }

    /// 提交用户输入
    pub async fn submit(&mut self, input: String) {
        self.streaming = true;
        let (tx, mut rx) = broadcast::channel(1024);
        self.event_rx = rx;

        let engine_input = input.clone();
        self.messages.push(format!("> {input}"));

        // 克隆需要的引用
        let config = self.config.clone();
        let mut session = deepcoder_engine::Session::new(config, std::sync::Arc::new(deepcoder_tools::ToolRouter::new()));

        // 运行回合
        tokio::spawn(async move {
            let _ = deepcoder_engine::turn::run_turn(&mut session, &engine_input, tx).await;
        });

        // 处理事件
        while let Ok(event) = self.event_rx.recv().await {
            match event {
                EngineEvent::TextDelta { content, .. } => {
                    if let Some(last) = self.messages.last_mut() {
                        last.push_str(&content);
                    }
                }
                EngineEvent::ReasoningDelta { content, .. } => {
                    self.reasoning.push_str(&content);
                }
                EngineEvent::TurnComplete { .. } => {
                    self.streaming = false;
                    break;
                }
                EngineEvent::Error { message, .. } => {
                    self.messages.push(format!("❌ {message}"));
                    self.streaming = false;
                    break;
                }
                _ => {}
            }
        }
    }
}
