//! Provider 抽象模型类型

use serde::{Deserialize, Serialize};
use super::tool::ToolSpec;

/// Provider 能力声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub tool_use: bool,
    pub vision: bool,
    pub reasoning: bool,
    pub streaming: bool,
    pub max_context_window: u32,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            tool_use: true,
            vision: false,
            reasoning: true,
            streaming: true,
            max_context_window: 131072,
        }
    }
}

/// Provider 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub capabilities: ProviderCapabilities,
}

/// 聊天请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub stream: bool,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// 聊天消息（序列化格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value,
}

/// SSE 流式事件
#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    Done,
    Error(String),
}
