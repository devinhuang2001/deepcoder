//! 配置模型类型

use serde::{Deserialize, Serialize};

/// 沙箱模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SandboxMode {
    /// 自动（平台支持时启用）
    #[serde(rename = "auto")]
    Auto,
    /// 强制启用
    #[serde(rename = "enforce")]
    Enforce,
    /// 禁用
    #[serde(rename = "off")]
    Off,
}

impl Default for SandboxMode {
    fn default() -> Self { Self::Auto }
}
