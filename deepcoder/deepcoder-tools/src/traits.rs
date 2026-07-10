//! Tool trait 定义

use async_trait::async_trait;
use deepcoder_error::DeepCoderResult;
use deepcoder_types::tool::*;

use crate::router::ToolRouter;

/// 工具审批请求。
#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub tool_name: String,
    pub message: String,
    pub arguments: serde_json::Value,
}

/// 交互式审批入口。
#[async_trait]
pub trait ToolApprover: Send + Sync {
    async fn approve(&self, request: ToolApprovalRequest) -> bool;
}

/// 工具执行上下文
#[derive(Clone)]
pub struct ToolContext {
    pub config: deepcoder_config::Config,
    pub workspace_root: Option<std::path::PathBuf>,
    pub tool_router: Option<std::sync::Arc<ToolRouter>>,
    pub tool_approver: Option<std::sync::Arc<dyn ToolApprover>>,
}

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
    async fn call(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput>;

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
