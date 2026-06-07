//! 工具系统核心类型
//!
//! 定义了 Tool trait、ToolSpec、ToolExposure 等核心抽象。
//! 借鉴 Codex CLI 的 ToolExecutor trait + Claude Code 的权限/并发模型。

use serde::{Deserialize, Serialize};

/// 工具向模型暴露的方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolExposure {
    /// 直接暴露在初始工具列表中
    Direct,
    /// 注册用于后续发现，但不在初始列表中
    Deferred,
    /// 仅直接暴露（不在 code-mode 嵌套中）
    DirectModelOnly,
    /// 保持注册用于分发，但不暴露给模型
    Hidden,
}

impl ToolExposure {
    pub fn is_direct(self) -> bool {
        matches!(self, Self::Direct | Self::DirectModelOnly)
    }
}

/// 工具规范描述，与 API 序列化格式分离
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// 工具调用的目标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

/// 工具执行的输出
pub trait ToolOutput: Send + Sync {
    fn as_json(&self) -> serde_json::Value;
    fn is_error(&self) -> bool;
    fn content_text(&self) -> Option<String>;
}

/// JSON 工具输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonToolOutput {
    pub data: serde_json::Value,
    pub is_error: bool,
}

impl ToolOutput for JsonToolOutput {
    fn as_json(&self) -> serde_json::Value {
        self.data.clone()
    }
    fn is_error(&self) -> bool {
        self.is_error
    }
    fn content_text(&self) -> Option<String> {
        self.data.as_str().map(String::from)
    }
}

impl JsonToolOutput {
    pub fn success(value: serde_json::Value) -> Self {
        Self { data: value, is_error: false }
    }
    pub fn text(text: impl Into<String>) -> Self {
        Self { data: serde_json::Value::String(text.into()), is_error: false }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self { data: serde_json::Value::String(msg.into()), is_error: true }
    }
}

/// 工具搜索元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSearchInfo {
    pub name: String,
    pub description: String,
    pub search_terms: Vec<String>,
}

/// 权限检查结果
#[derive(Debug, Clone)]
pub enum PermissionResult {
    Allow,
    Deny { reason: String },
    Prompt { message: String },
}
