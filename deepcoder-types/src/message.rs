//! 消息类型定义

use serde::{Deserialize, Serialize};
use super::MessageId;

/// 消息角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
    Reasoning,
}

/// 消息内容类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        id: String,
        content: serde_json::Value,
        is_error: bool,
    },
    Reasoning {
        content: String,
    },
}

/// 消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub role: MessageRole,
    pub contents: Vec<ContentType>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub token_count: Option<u32>,
}

impl Message {
    pub fn new(role: MessageRole, contents: Vec<ContentType>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            role,
            contents,
            timestamp: chrono::Utc::now(),
            token_count: None,
        }
    }

    pub fn text(role: MessageRole, text: impl Into<String>) -> Self {
        Self::new(role, vec![ContentType::Text(text.into())])
    }

    pub fn reasoning(role: MessageRole, text: impl Into<String>) -> Self {
        Self::new(role, vec![ContentType::Reasoning { content: text.into() }])
    }
}
