//! DeepCoder 统一错误类型

use thiserror::Error;

/// DeepCoder 核心错误类型
#[derive(Error, Debug)]
pub enum DeepCoderError {
    #[error("配置错误: {0}")]
    Config(String),

    #[error("Provider 错误: {0}")]
    Provider(String),

    #[error("API 请求失败: {0}")]
    Api(#[from] reqwest::Error),

    #[error("工具未找到: {0}")]
    ToolNotFound(String),

    #[error("工具执行失败: {0}")]
    ToolExecution(String),

    #[error("工具调用被拒绝: {reason}")]
    ToolDenied { reason: String },

    #[error("MCP 错误: {0}")]
    Mcp(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("会话错误: {0}")]
    Session(String),

    #[error("回合错误: {0}")]
    Turn(String),

    #[error("扩展错误: {0}")]
    Extension(String),

    #[error("内部错误: {0}")]
    Internal(String),
}

pub type DeepCoderResult<T> = Result<T, DeepCoderError>;

impl From<String> for DeepCoderError {
    fn from(s: String) -> Self {
        DeepCoderError::Internal(s)
    }
}

impl From<&str> for DeepCoderError {
    fn from(s: &str) -> Self {
        DeepCoderError::Internal(s.to_string())
    }
}
