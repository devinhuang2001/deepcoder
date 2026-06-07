//! 会话/线程类型定义

use serde::{Deserialize, Serialize};
use super::{SessionId, ThreadId, TurnId, Message};

/// 线程状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: ThreadId,
    pub model: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub message_count: u32,
    pub token_usage: TokenUsage,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Token 使用统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_cost: f64,
}

impl TokenUsage {
    pub fn add(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.reasoning_tokens += other.reasoning_tokens;
        self.total_cost += other.total_cost;
    }
}

/// 回合上下文
#[derive(Debug, Clone)]
pub struct TurnContext {
    pub turn_id: TurnId,
    pub thread_id: ThreadId,
    pub session_id: SessionId,
    pub messages: Vec<Message>,
    pub model: String,
    pub max_tool_iterations: u32,
}
