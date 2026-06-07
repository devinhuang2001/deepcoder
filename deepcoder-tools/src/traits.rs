//! Tool trait 定义

use async_trait::async_trait;
use deepcoder_types::tool::*;
use deepcoder_error::DeepCoderResult;
use super::router::ToolContext;

/// 核心 Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具唯一名称
    fn name(&self) -> &'static str;

    /// 工具规范描述
    fn spec(&self) -> ToolSpec;

    /// 工具可见性
    fn exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    /// 执行工具
    async fn call(&self, params: serde_json::Value, ctx: &ToolContext) -> DeepCoderResult<JsonToolOutput>;

    /// 权限检查（可选覆盖）
    fn check_permissions(&self, _input: &serde_json::Value) -> PermissionResult {
        PermissionResult::Allow
    }

    /// 是否可并发执行
    fn is_concurrency_safe(&self) -> bool {
        false
    }

    /// 搜索元数据（用于 Deferred 工具）
    fn search_info(&self) -> Option<ToolSearchInfo> {
        None
    }
}
