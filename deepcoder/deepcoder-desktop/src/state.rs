use deepcoder_persistence::index::SessionEntry;
use deepcoder_tools::traits::ToolApprovalRequest;
use deepcoder_types::event::EngineEvent;
use deepcoder_types::message::{ContentType, Message, MessageRole};
use deepcoder_types::session::TokenUsage;
use tokio::sync::oneshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    System,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolActivity {
    pub name: String,
    pub detail: String,
    pub status: String,
    pub is_error: bool,
}

pub struct DesktopApprovalPrompt {
    pub tool_name: String,
    pub message: String,
    pub command: Option<String>,
    pub arguments: serde_json::Value,
    respond_to: Option<oneshot::Sender<bool>>,
}

impl DesktopApprovalPrompt {
    pub fn new(request: ToolApprovalRequest, respond_to: oneshot::Sender<bool>) -> Self {
        let command = request
            .arguments
            .get("command")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);
        Self {
            tool_name: request.tool_name,
            message: request.message,
            command,
            arguments: request.arguments,
            respond_to: Some(respond_to),
        }
    }

    #[cfg(test)]
    fn test_prompt(respond_to: oneshot::Sender<bool>) -> Self {
        Self {
            tool_name: "bash".into(),
            message: "approve command".into(),
            command: Some("Get-Date".into()),
            arguments: serde_json::json!({"command": "Get-Date"}),
            respond_to: Some(respond_to),
        }
    }

    pub fn resolve(&mut self, approved: bool) {
        if let Some(respond_to) = self.respond_to.take() {
            let _ = respond_to.send(approved);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigWizardState {
    pub visible: bool,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub error: Option<String>,
}

pub struct DesktopAppState {
    pub config: deepcoder_config::Config,
    pub config_path: std::path::PathBuf,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub reasoning: String,
    pub tools: Vec<ToolActivity>,
    pub token_usage: TokenUsage,
    pub sessions: Vec<SessionEntry>,
    pub selected_session: Option<usize>,
    pub status: String,
    pub streaming: bool,
    pub wizard: ConfigWizardState,
    pub pending_approval: Option<DesktopApprovalPrompt>,
    pub approval_queue: std::collections::VecDeque<DesktopApprovalPrompt>,
}

impl DesktopAppState {
    pub fn new(config: deepcoder_config::Config, env_api_key: Option<&str>) -> Self {
        let wizard_visible = needs_api_key_wizard(&config, env_api_key);
        Self {
            config_path: deepcoder_config::Config::global_config_path(),
            wizard: ConfigWizardState {
                visible: wizard_visible,
                api_key: String::new(),
                model: config.provider.model.clone(),
                base_url: config.provider.base_url.clone(),
                error: None,
            },
            config,
            messages: Vec::new(),
            input: String::new(),
            reasoning: String::new(),
            tools: Vec::new(),
            token_usage: TokenUsage::default(),
            sessions: Vec::new(),
            selected_session: None,
            status: if wizard_visible {
                "请先完成 API Key 配置".into()
            } else {
                "Ready".into()
            },
            streaming: false,
            pending_approval: None,
            approval_queue: Default::default(),
        }
    }

    pub fn refresh_sessions(&mut self) {
        let persistence =
            deepcoder_persistence::Persistence::new(self.config.system.data_dir.clone());
        self.sessions = persistence.list_sessions();
        if self.sessions.is_empty() {
            self.selected_session = None;
        } else {
            self.selected_session = Some(
                self.selected_session
                    .unwrap_or_default()
                    .min(self.sessions.len().saturating_sub(1)),
            );
        }
    }

    pub fn save_wizard_config(&mut self) -> anyhow::Result<()> {
        let api_key = self.wizard.api_key.trim();
        if api_key.is_empty() {
            anyhow::bail!("API Key 不能为空");
        }
        let model = self.wizard.model.trim();
        let base_url = self.wizard.base_url.trim();
        if model.is_empty() {
            anyhow::bail!("模型名称不能为空");
        }
        if base_url.is_empty() {
            anyhow::bail!("Base URL 不能为空");
        }

        deepcoder_config::Config::set_value_at_path(&self.config_path, "api_key", api_key)?;
        deepcoder_config::Config::set_value_at_path(&self.config_path, "provider.model", model)?;
        deepcoder_config::Config::set_value_at_path(
            &self.config_path,
            "provider.base_url",
            base_url,
        )?;
        self.config = deepcoder_config::Config::load_from_paths(
            self.config_path.clone(),
            std::path::PathBuf::from(".deepcoder/config.toml"),
        )?;
        self.wizard.visible = false;
        self.wizard.error = None;
        self.status = "配置已保存".into();
        Ok(())
    }

    pub fn begin_user_message(&mut self, input: String) {
        self.streaming = true;
        self.reasoning.clear();
        self.status = "Streaming...".into();
        self.messages.push(ChatMessage {
            role: ChatRole::User,
            content: input,
        });
        self.messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
        });
    }

    pub fn apply_engine_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::TurnStart { .. } => {
                self.streaming = true;
                self.status = "Turn started".into();
            }
            EngineEvent::TextDelta { content, .. } => {
                self.ensure_assistant_tail();
                if let Some(last) = self.messages.last_mut() {
                    last.content.push_str(&content);
                }
            }
            EngineEvent::ReasoningDelta { content, .. } => {
                self.reasoning.push_str(&content);
            }
            EngineEvent::ToolCallRequested { tool_call, .. } => {
                self.tools.push(ToolActivity {
                    name: tool_call.tool_name,
                    detail: tool_call.arguments.to_string(),
                    status: "requested".into(),
                    is_error: false,
                });
            }
            EngineEvent::ToolResult {
                tool_call_id,
                result,
                is_error,
                ..
            } => {
                self.tools.push(ToolActivity {
                    name: tool_call_id,
                    detail: result.to_string(),
                    status: if is_error { "error" } else { "done" }.into(),
                    is_error,
                });
            }
            EngineEvent::TurnComplete { token_usage, .. } => {
                self.token_usage = token_usage;
                self.streaming = false;
                self.status = "Ready".into();
            }
            EngineEvent::Error { message, .. } => {
                self.messages.push(ChatMessage {
                    role: ChatRole::Error,
                    content: message,
                });
                self.streaming = false;
                self.status = "Error".into();
            }
        }
    }

    pub fn queue_approval(&mut self, prompt: DesktopApprovalPrompt) {
        self.approval_queue.push_back(prompt);
        if self.pending_approval.is_none() {
            self.pending_approval = self.approval_queue.pop_front();
        }
        self.status = "等待工具审批".into();
    }

    pub fn resolve_pending_approval(&mut self, approved: bool) {
        if let Some(mut prompt) = self.pending_approval.take() {
            let tool_name = prompt.tool_name.clone();
            prompt.resolve(approved);
            self.tools.push(ToolActivity {
                name: tool_name,
                detail: if approved { "approved" } else { "denied" }.into(),
                status: "approval".into(),
                is_error: !approved,
            });
        }
        self.pending_approval = self.approval_queue.pop_front();
    }

    pub fn load_messages_from_session(&mut self, messages: &[Message]) {
        self.messages = messages
            .iter()
            .filter_map(chat_message_from_message)
            .collect();
        self.reasoning.clear();
        self.tools.clear();
        self.status = "Session loaded".into();
    }

    fn ensure_assistant_tail(&mut self) {
        if !matches!(
            self.messages.last(),
            Some(ChatMessage {
                role: ChatRole::Assistant,
                ..
            })
        ) {
            self.messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: String::new(),
            });
        }
    }
}

pub fn needs_api_key_wizard(config: &deepcoder_config::Config, env_api_key: Option<&str>) -> bool {
    config
        .api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .is_none()
        && env_api_key
            .filter(|value| !value.trim().is_empty())
            .is_none()
}

fn chat_message_from_message(message: &Message) -> Option<ChatMessage> {
    let content = message_text(message);
    if content.is_empty() {
        return None;
    }
    let role = match message.role {
        MessageRole::User => ChatRole::User,
        MessageRole::Assistant => ChatRole::Assistant,
        MessageRole::System => ChatRole::System,
        MessageRole::Tool => return None,
    };
    Some(ChatMessage { role, content })
}

fn message_text(message: &Message) -> String {
    message
        .contents
        .iter()
        .filter_map(|content| match content {
            ContentType::Text(text) => Some(text.as_str()),
            ContentType::Reasoning { content } => Some(content.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(name: &str) -> deepcoder_config::Config {
        let mut config = deepcoder_config::Config::load_default().unwrap();
        config.system.data_dir = std::env::temp_dir().join(format!(
            "deepcoder_desktop_state_{name}_{}",
            uuid::Uuid::new_v4()
        ));
        config.api_key = None;
        config
    }

    #[test]
    fn config_wizard_required_when_key_missing() {
        let config = config("missing");
        assert!(needs_api_key_wizard(&config, None));
        assert!(!needs_api_key_wizard(&config, Some("sk-test")));
    }

    #[test]
    fn save_wizard_config_updates_file_and_current_config() {
        let mut state = DesktopAppState::new(config("save"), None);
        let dir =
            std::env::temp_dir().join(format!("deepcoder_desktop_config_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        state.config_path = dir.join("config.toml");
        state.wizard.api_key = "sk-test".into();
        state.wizard.model = "deepseek-reasoner".into();
        state.wizard.base_url = "https://api.deepseek.com".into();

        state.save_wizard_config().unwrap();

        assert!(!state.wizard.visible);
        assert_eq!(state.config.api_key.as_deref(), Some("sk-test"));
        assert_eq!(state.config.provider.model, "deepseek-reasoner");
    }

    #[test]
    fn reducer_applies_stream_events() {
        let mut state = DesktopAppState::new(config("events"), Some("sk"));
        state.begin_user_message("hello".into());
        state.apply_engine_event(EngineEvent::TextDelta {
            thread_id: uuid::Uuid::new_v4(),
            content: "answer".into(),
        });
        state.apply_engine_event(EngineEvent::ReasoningDelta {
            thread_id: uuid::Uuid::new_v4(),
            content: "think".into(),
        });
        state.apply_engine_event(EngineEvent::TurnComplete {
            thread_id: uuid::Uuid::new_v4(),
            turn_id: uuid::Uuid::new_v4(),
            token_usage: TokenUsage {
                input_tokens: 1,
                output_tokens: 2,
                reasoning_tokens: 3,
                total_cost: 0.0,
            },
        });

        assert_eq!(state.messages.last().unwrap().content, "answer");
        assert_eq!(state.reasoning, "think");
        assert!(!state.streaming);
        assert_eq!(state.token_usage.output_tokens, 2);
    }

    #[tokio::test]
    async fn approval_flow_returns_user_decision() {
        let mut state = DesktopAppState::new(config("approval"), Some("sk"));
        let (tx, rx) = oneshot::channel();
        state.queue_approval(DesktopApprovalPrompt::test_prompt(tx));
        assert!(state.pending_approval.is_some());
        state.resolve_pending_approval(true);
        assert!(rx.await.unwrap());
        assert!(state.pending_approval.is_none());
        assert_eq!(state.tools.last().unwrap().detail, "approved");
    }
}
