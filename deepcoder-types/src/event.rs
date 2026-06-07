//! 事件类型定义 — 流式引擎事件

use serde::{Deserialize, Serialize};
use super::{ThreadId, TurnId, tool::ToolCall};

/// 引擎发出的流式事件
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// 回合开始
    TurnStart {
        thread_id: ThreadId,
        turn_id: TurnId,
    },
    /// 文本增量
    TextDelta {
        thread_id: ThreadId,
        content: String,
    },
    /// 推理增量（DeepSeek V4 reasoning_content）
    ReasoningDelta {
        thread_id: ThreadId,
        content: String,
    },
    /// 工具调用
    ToolCallRequested {
        thread_id: ThreadId,
        tool_call: ToolCall,
    },
    /// 工具结果
    ToolResult {
        thread_id: ThreadId,
        tool_call_id: String,
        result: serde_json::Value,
        is_error: bool,
    },
    /// 回合完成
    TurnComplete {
        thread_id: ThreadId,
        turn_id: TurnId,
        token_usage: super::session::TokenUsage,
    },
    /// 错误
    Error {
        thread_id: ThreadId,
        message: String,
    },
}
