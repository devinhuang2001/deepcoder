//! ToolRouter — 工具注册和路由分发

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;
use deepcoder_types::tool::*;
use deepcoder_error::DeepCoderResult;

use deepcoder_config::Config;
use super::traits::Tool;

/// 工具执行上下文
pub struct ToolContext {
    pub config: deepcoder_config::Config,
    pub tool_router: Option<Arc<ToolRouter>>,
}

/// 工具路由
pub struct ToolRouter {
    tools: RwLock<HashMap<&'static str, Arc<dyn Tool>>>,
}

impl ToolRouter {
    pub fn new() -> Self {
        Self { tools: RwLock::new(HashMap::new()) }
    }

    /// 注册工具
    pub async fn register(&self, tool: Arc<dyn Tool>) {
        self.tools.write().await.insert(tool.name(), tool);
    }

    /// 批量注册
    pub async fn register_all(&self, tools: Vec<Arc<dyn Tool>>) {
        let mut map = self.tools.write().await;
        for tool in tools {
            map.insert(tool.name(), tool);
        }
    }

    /// 获取所有工具 spec
    pub async fn all_specs(&self) -> Vec<ToolSpec> {
        self.tools.read().await.values()
            .map(|t| t.spec())
            .collect()
    }

    /// 获取直接可见的工具 spec
    pub async fn direct_specs(&self) -> Vec<ToolSpec> {
        self.tools.read().await.values()
            .filter(|t| t.exposure().is_direct())
            .map(|t| t.spec())
            .collect()
    }

    /// 执行工具
    pub async fn execute(&self, call: &ToolCall, ctx: &ToolContext) -> DeepCoderResult<JsonToolOutput> {
        let tools = self.tools.read().await;
        let tool = tools.get(call.tool_name.as_str())
            .ok_or_else(|| DeepCoderError::ToolNotFound(call.tool_name.clone()))?;

        // 权限检查
        match tool.check_permissions(&call.arguments) {
            PermissionResult::Deny { reason } => {
                return Err(DeepCoderError::ToolDenied { reason });
            }
            PermissionResult::Prompt { message } => {
                // TODO: 集成审批 UI
                tracing::warn!("工具需要审批: {}", message);
            }
            PermissionResult::Allow => {}
        }

        tool.call(call.arguments.clone(), ctx).await
    }
}
