//! 会话管理

use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use deepcoder_types::message::*;
use deepcoder_types::session::*;
use deepcoder_types::DeepCoderResult} as _;

/// 会话
pub struct Session {
    pub id: Uuid,
    pub thread: Thread,
    pub config: deepcoder_config::Config,
    pub messages: Vec<Message>,
    pub tool_router: Arc<deepcoder_tools::ToolRouter>,
}

impl Session {
    pub fn new(config: deepcoder_config::Config, tool_router: Arc<deepcoder_tools::ToolRouter>) -> Self {
        Self {
            id: Uuid::new_v4(),
            thread: Thread {
                id: Uuid::new_v4(),
                model: config.provider.model.clone(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                message_count: 0,
                token_usage: TokenUsage::default(),
                metadata: std::collections::HashMap::new(),
            },
            config,
            messages: Vec::new(),
            tool_router,
        }
    }

    /// 添加消息到历史
    pub fn add_message(&mut self, msg: Message) {
        self.thread.message_count += 1;
        self.thread.updated_at = chrono::Utc::now();
        self.messages.push(msg);
    }

    /// 获取当前 token 使用统计
    pub fn token_usage(&self) -> &TokenUsage {
        &self.thread.token_usage
    }

    /// 获取可见消息列表（排除推理 token）
    pub fn visible_messages(&self) -> Vec<&Message> {
        self.messages.iter()
            .filter(|m| !matches!(m.role, MessageRole::Tool))
            .collect()
    }
}
