//! TUI 主应用

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use deepcoder_persistence::index::SessionEntry;
use deepcoder_tools::traits::{ToolApprovalRequest, ToolApprover};
use deepcoder_types::event::EngineEvent;
use tokio::sync::{
    broadcast::{self, error::TryRecvError},
    mpsc, oneshot,
};
use tokio::task::JoinHandle;

pub struct ApprovalPrompt {
    pub tool_name: String,
    pub message: String,
    pub command: Option<String>,
    respond_to: Option<oneshot::Sender<bool>>,
}

pub struct ApprovalView<'a> {
    pub tool_name: &'a str,
    pub message: &'a str,
    pub command: Option<&'a str>,
}

pub struct SessionPickerState {
    pub sessions: Vec<SessionEntry>,
    pub selected: usize,
}

pub struct SessionPickerView<'a> {
    pub sessions: &'a [SessionEntry],
    pub selected: usize,
}

#[derive(Clone)]
struct TuiApprover {
    tx: mpsc::UnboundedSender<ApprovalPrompt>,
}

#[async_trait]
impl ToolApprover for TuiApprover {
    async fn approve(&self, request: ToolApprovalRequest) -> bool {
        let (respond_to, response) = oneshot::channel();
        let prompt = ApprovalPrompt {
            tool_name: request.tool_name,
            message: request.message,
            command: request
                .arguments
                .get("command")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            respond_to: Some(respond_to),
        };

        if self.tx.send(prompt).is_err() {
            return false;
        }

        response.await.unwrap_or(false)
    }
}

/// TUI 应用状态
pub struct App {
    config: deepcoder_config::Config,
    session: Option<deepcoder_engine::Session>,
    event_rx: broadcast::Receiver<EngineEvent>,
    approval_tx: mpsc::UnboundedSender<ApprovalPrompt>,
    approval_rx: mpsc::UnboundedReceiver<ApprovalPrompt>,
    approval_queue: VecDeque<ApprovalPrompt>,
    turn_task: Option<JoinHandle<(deepcoder_engine::Session, std::result::Result<(), String>)>>,
    /// 聊天消息
    pub messages: Vec<String>,
    /// 输入缓冲区
    pub input: String,
    history: Vec<String>,
    history_index: Option<usize>,
    /// 推理内容
    pub reasoning: String,
    /// 是否正在流式响应
    pub streaming: bool,
    /// 是否显示推理面板
    pub show_reasoning: bool,
    pub pending_approval: Option<ApprovalPrompt>,
    pub session_picker: Option<SessionPickerState>,
    should_quit: bool,
}

impl App {
    pub fn new(config: deepcoder_config::Config) -> Self {
        let (_tx, rx) = broadcast::channel(1024);
        let (approval_tx, approval_rx) = mpsc::unbounded_channel();

        Self {
            session: Some(Self::new_session(&config, approval_tx.clone())),
            config,
            event_rx: rx,
            approval_tx,
            approval_rx,
            approval_queue: VecDeque::new(),
            turn_task: None,
            messages: Vec::new(),
            input: String::new(),
            history: Vec::new(),
            history_index: None,
            reasoning: String::new(),
            streaming: false,
            show_reasoning: true,
            pending_approval: None,
            session_picker: None,
            should_quit: false,
        }
    }

    pub async fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            self.drain_engine_events();
            self.drain_approval_requests();
            self.finish_turn_if_ready().await;
            terminal.draw(|frame| crate::render::render(self, frame))?;

            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key.code, key.modifiers).await?;
            }
        }

        Ok(())
    }

    fn new_session(
        config: &deepcoder_config::Config,
        approval_tx: mpsc::UnboundedSender<ApprovalPrompt>,
    ) -> deepcoder_engine::Session {
        let tool_router = std::sync::Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let mut session = deepcoder_engine::Session::new(config.clone(), tool_router);
        session.set_tool_approver(Some(Arc::new(TuiApprover { tx: approval_tx })));
        session
    }

    async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if self.pending_approval.is_some() {
            match code {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.resolve_pending_approval(true);
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.resolve_pending_approval(false);
                }
                _ => {}
            }
            return Ok(());
        }

        if self.session_picker.is_some() {
            self.handle_session_picker_key(code);
            return Ok(());
        }

        match code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) && !self.streaming => {
                self.open_session_picker();
            }
            KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) && !self.streaming => {
                self.input.push('\n');
                self.history_index = None;
            }
            KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) && !self.streaming => {
                self.input.push('\n');
                self.history_index = None;
            }
            KeyCode::Enter if !self.streaming => {
                let input = self.input.trim().to_string();
                self.input.clear();
                self.history_index = None;
                if !input.is_empty() {
                    self.push_history(input.clone());
                    self.submit(input).await;
                }
            }
            KeyCode::Up if !self.streaming => {
                self.history_prev();
            }
            KeyCode::Down if !self.streaming => {
                self.history_next();
            }
            KeyCode::Backspace if !self.streaming => {
                self.input.pop();
                self.history_index = None;
            }
            KeyCode::Char(ch) if !self.streaming => {
                self.input.push(ch);
                self.history_index = None;
            }
            _ => {}
        }

        Ok(())
    }

    fn push_history(&mut self, input: String) {
        if self.history.last() != Some(&input) {
            self.history.push(input);
        }
    }

    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let index = self
            .history_index
            .map(|index| index.saturating_sub(1))
            .unwrap_or_else(|| self.history.len() - 1);
        self.history_index = Some(index);
        self.input = self.history[index].clone();
    }

    fn history_next(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 >= self.history.len() {
            self.history_index = None;
            self.input.clear();
        } else {
            let index = index + 1;
            self.history_index = Some(index);
            self.input = self.history[index].clone();
        }
    }

    fn open_session_picker(&mut self) {
        let sessions = deepcoder_persistence::Persistence::new(self.config.system.data_dir.clone())
            .list_sessions();
        if sessions.is_empty() {
            self.push_error("没有可恢复的 session");
            return;
        }
        self.session_picker = Some(SessionPickerState {
            sessions,
            selected: 0,
        });
    }

    fn handle_session_picker_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.session_picker = None,
            KeyCode::Up => {
                if let Some(picker) = self.session_picker.as_mut() {
                    picker.selected = picker.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(picker) = self.session_picker.as_mut() {
                    let last = picker.sessions.len().saturating_sub(1);
                    picker.selected = (picker.selected + 1).min(last);
                }
            }
            KeyCode::Enter => self.load_selected_session(false),
            KeyCode::Char('f') | KeyCode::Char('F') => self.load_selected_session(true),
            _ => {}
        }
    }

    fn load_selected_session(&mut self, fork: bool) {
        let Some(picker) = self.session_picker.take() else {
            return;
        };
        let Some(entry) = picker.sessions.get(picker.selected) else {
            return;
        };
        let Ok(session_id) = entry.id.parse::<uuid::Uuid>() else {
            self.push_error("session id 无效");
            return;
        };
        let persistence =
            deepcoder_persistence::Persistence::new(self.config.system.data_dir.clone());
        let router = Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let loaded = if fork {
            deepcoder_engine::Session::fork_from(
                self.config.clone(),
                router,
                &persistence,
                session_id,
            )
        } else {
            deepcoder_engine::Session::resume(self.config.clone(), router, &persistence, session_id)
        };
        match loaded {
            Ok(Some(mut session)) => {
                session.set_tool_approver(Some(Arc::new(TuiApprover {
                    tx: self.approval_tx.clone(),
                })));
                let id = session.id;
                self.session = Some(session);
                let action = if fork { "Forked" } else { "Resumed" };
                self.messages.push(format!("{action} session: {id}"));
            }
            Ok(None) => self.push_error("session 不存在"),
            Err(error) => self.push_error(&format!("session 加载失败: {error}")),
        }
    }

    /// 提交用户输入
    pub async fn submit(&mut self, input: String) {
        self.streaming = true;
        let (tx, rx) = broadcast::channel(1024);
        self.event_rx = rx;

        let engine_input = input.clone();
        self.messages.push(format!("> {input}"));
        self.messages.push(String::new());

        let Some(mut session) = self.session.take() else {
            self.push_error("上一轮还在收尾，稍后再试");
            self.streaming = false;
            return;
        };

        self.turn_task = Some(tokio::spawn(async move {
            let result = deepcoder_engine::turn::run_turn(&mut session, &engine_input, tx)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string());
            (session, result)
        }));
    }

    fn drain_engine_events(&mut self) {
        loop {
            match self.event_rx.try_recv() {
                Ok(event) => self.handle_engine_event(event),
                Err(TryRecvError::Empty | TryRecvError::Closed) => break,
                Err(TryRecvError::Lagged(_)) => continue,
            }
        }
    }

    fn drain_approval_requests(&mut self) {
        while let Ok(prompt) = self.approval_rx.try_recv() {
            self.approval_queue.push_back(prompt);
        }
        if self.pending_approval.is_none() {
            self.pending_approval = self.approval_queue.pop_front();
        }
    }

    fn resolve_pending_approval(&mut self, approved: bool) {
        let Some(mut prompt) = self.pending_approval.take() else {
            return;
        };
        if let Some(respond_to) = prompt.respond_to.take() {
            let _ = respond_to.send(approved);
        }
        let decision = if approved { "approved" } else { "denied" };
        self.messages
            .push(format!("Approval {decision}: {}", prompt.tool_name));
        self.pending_approval = self.approval_queue.pop_front();
    }

    pub fn approval_view(&self) -> Option<ApprovalView<'_>> {
        self.pending_approval.as_ref().map(|prompt| ApprovalView {
            tool_name: &prompt.tool_name,
            message: &prompt.message,
            command: prompt.command.as_deref(),
        })
    }

    pub fn session_picker_view(&self) -> Option<SessionPickerView<'_>> {
        self.session_picker
            .as_ref()
            .map(|picker| SessionPickerView {
                sessions: &picker.sessions,
                selected: picker.selected,
            })
    }

    fn handle_engine_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::TextDelta { content, .. } => {
                if let Some(last) = self.messages.last_mut() {
                    last.push_str(&content);
                }
            }
            EngineEvent::ReasoningDelta { content, .. } => {
                self.reasoning.push_str(&content);
            }
            EngineEvent::Error { message, .. } => {
                self.push_error(&message);
            }
            _ => {}
        }
    }

    async fn finish_turn_if_ready(&mut self) {
        let Some(task) = self.turn_task.as_ref() else {
            return;
        };
        if !task.is_finished() {
            return;
        }

        let task = self.turn_task.take().expect("checked task exists");
        match task.await {
            Ok((session, Ok(()))) => {
                self.session = Some(session);
                self.streaming = false;
            }
            Ok((session, Err(error))) => {
                self.session = Some(session);
                self.push_error(&error);
                self.streaming = false;
            }
            Err(error) => {
                self.session = Some(Self::new_session(&self.config, self.approval_tx.clone()));
                self.push_error(&format!("turn task failed: {error}"));
                self.streaming = false;
            }
        }
    }

    fn push_error(&mut self, message: &str) {
        let line = format!("Error: {message}");
        if self.messages.last().is_none_or(|last| last != &line) {
            self.messages.push(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> deepcoder_config::Config {
        let mut config = deepcoder_config::Config::load_default().unwrap();
        config.system.data_dir = std::env::temp_dir().join(format!(
            "deepcoder_tui_app_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        config
    }

    #[test]
    fn app_initial_state() {
        let app = App::new(config());
        assert!(app.messages.is_empty());
        assert!(app.input.is_empty());
        assert!(app.reasoning.is_empty());
        assert!(!app.streaming);
    }

    #[tokio::test]
    async fn handle_key_appends_char_and_backspace() {
        let mut app = App::new(config());
        app.handle_key(KeyCode::Char('a'), KeyModifiers::empty())
            .await
            .unwrap();
        app.handle_key(KeyCode::Char('b'), KeyModifiers::empty())
            .await
            .unwrap();
        assert_eq!(app.input, "ab");
        app.handle_key(KeyCode::Backspace, KeyModifiers::empty())
            .await
            .unwrap();
        assert_eq!(app.input, "a");
    }

    #[tokio::test]
    async fn handle_key_supports_multiline_input() {
        let mut app = App::new(config());
        app.handle_key(KeyCode::Char('a'), KeyModifiers::empty())
            .await
            .unwrap();
        app.handle_key(KeyCode::Enter, KeyModifiers::SHIFT)
            .await
            .unwrap();
        app.handle_key(KeyCode::Char('b'), KeyModifiers::empty())
            .await
            .unwrap();
        assert_eq!(app.input, "a\nb");
    }

    #[tokio::test]
    async fn input_history_moves_up_and_down() {
        let mut app = App::new(config());
        app.push_history("first".into());
        app.push_history("second".into());

        app.handle_key(KeyCode::Up, KeyModifiers::empty())
            .await
            .unwrap();
        assert_eq!(app.input, "second");
        app.handle_key(KeyCode::Up, KeyModifiers::empty())
            .await
            .unwrap();
        assert_eq!(app.input, "first");
        app.handle_key(KeyCode::Down, KeyModifiers::empty())
            .await
            .unwrap();
        assert_eq!(app.input, "second");
        app.handle_key(KeyCode::Down, KeyModifiers::empty())
            .await
            .unwrap();
        assert!(app.input.is_empty());
    }

    #[test]
    fn tui_streaming_updates_chat_and_reasoning() {
        let mut app = App::new(config());
        app.messages.push(String::new());
        app.handle_engine_event(EngineEvent::TextDelta {
            thread_id: uuid::Uuid::new_v4(),
            content: "hello".into(),
        });
        app.handle_engine_event(EngineEvent::ReasoningDelta {
            thread_id: uuid::Uuid::new_v4(),
            content: "thinking".into(),
        });
        assert_eq!(app.messages.last().unwrap(), "hello");
        assert_eq!(app.reasoning, "thinking");
    }

    #[test]
    fn tui_provider_error_restores_input_state() {
        let mut app = App::new(config());
        app.streaming = true;
        app.handle_engine_event(EngineEvent::Error {
            thread_id: uuid::Uuid::new_v4(),
            message: "provider failed".into(),
        });
        assert!(app.messages.last().unwrap().contains("provider failed"));
    }

    #[tokio::test]
    async fn approval_prompt_accepts_and_advances_queue() {
        let mut app = App::new(config());
        let (first_tx, first_rx) = oneshot::channel();
        let (second_tx, _second_rx) = oneshot::channel();
        app.pending_approval = Some(ApprovalPrompt {
            tool_name: "bash".into(),
            message: "approve first".into(),
            command: Some("Get-Date".into()),
            respond_to: Some(first_tx),
        });
        app.approval_queue.push_back(ApprovalPrompt {
            tool_name: "agent".into(),
            message: "approve second".into(),
            command: None,
            respond_to: Some(second_tx),
        });

        app.handle_key(KeyCode::Char('y'), KeyModifiers::empty())
            .await
            .unwrap();
        assert!(first_rx.await.unwrap());
        assert_eq!(app.pending_approval.as_ref().unwrap().tool_name, "agent");
        assert!(app.messages.last().unwrap().contains("approved"));
    }

    #[tokio::test]
    async fn tui_approver_sends_prompt_and_receives_denial() {
        let mut app = App::new(config());
        let approver = TuiApprover {
            tx: app.approval_tx.clone(),
        };
        let handle = tokio::spawn(async move {
            approver
                .approve(ToolApprovalRequest {
                    tool_name: "bash".into(),
                    message: "command requires approval".into(),
                    arguments: serde_json::json!({"command": "Get-Date"}),
                })
                .await
        });

        tokio::task::yield_now().await;
        app.drain_approval_requests();
        let approval = app.approval_view().unwrap();
        assert_eq!(approval.tool_name, "bash");
        assert_eq!(approval.command, Some("Get-Date"));

        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty())
            .await
            .unwrap();
        assert!(!handle.await.unwrap());
    }

    #[tokio::test]
    async fn session_picker_resumes_persisted_session() {
        let config = config();
        let persistence = deepcoder_persistence::Persistence::new(config.system.data_dir.clone());
        let router = Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let mut session = deepcoder_engine::Session::new(config.clone(), router);
        let message = deepcoder_types::message::Message::text(
            deepcoder_types::message::MessageRole::User,
            "restore me",
        );
        session.add_message(message.clone());
        persistence
            .record_message(&session.id, &session.thread, &message)
            .unwrap();

        let mut app = App::new(config);
        app.handle_key(KeyCode::Char('s'), KeyModifiers::CONTROL)
            .await
            .unwrap();
        assert!(app.session_picker.is_some());
        app.handle_key(KeyCode::Enter, KeyModifiers::empty())
            .await
            .unwrap();
        assert!(app.session_picker.is_none());
        assert!(
            app.messages
                .last()
                .unwrap()
                .contains(&session.id.to_string())
        );
    }
}
