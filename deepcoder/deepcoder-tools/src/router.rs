//! ToolRouter — 工具注册和路由分发

use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_types::tool::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::traits::{Tool, ToolApprovalRequest, ToolContext};

/// 工具路由
pub struct ToolRouter {
    tools: RwLock<HashMap<&'static str, Arc<dyn Tool>>>,
}

impl ToolRouter {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_builtins() -> Self {
        let mut tools = HashMap::new();
        for tool in crate::builtin_tools() {
            tools.insert(tool.name(), tool);
        }
        Self {
            tools: RwLock::new(tools),
        }
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
        self.tools.read().await.values().map(|t| t.spec()).collect()
    }

    /// 获取直接可见的工具 spec
    pub async fn direct_specs(&self) -> Vec<ToolSpec> {
        self.tools
            .read()
            .await
            .values()
            .filter(|t| t.exposure().is_direct())
            .map(|t| t.spec())
            .collect()
    }

    /// 查询工具是否声明为并发安全。未知工具默认不并发。
    pub async fn is_concurrency_safe(&self, tool_name: &str) -> bool {
        self.tools
            .read()
            .await
            .get(tool_name)
            .is_some_and(|tool| tool.is_concurrency_safe())
    }

    /// 执行工具
    pub async fn execute(
        &self,
        call: &ToolCall,
        ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let tool = {
            let tools = self.tools.read().await;
            tools
                .get(call.tool_name.as_str())
                .cloned()
                .ok_or_else(|| DeepCoderError::ToolNotFound(call.tool_name.clone()))?
        };

        // 权限检查
        match tool.check_permissions(&call.arguments) {
            PermissionResult::Deny { reason } => {
                return Err(DeepCoderError::ToolDenied { reason });
            }
            PermissionResult::Prompt { message } => {
                let Some(approver) = ctx.tool_approver.as_ref() else {
                    return Err(DeepCoderError::ToolDenied { reason: message });
                };
                let approved = approver
                    .approve(ToolApprovalRequest {
                        tool_name: call.tool_name.clone(),
                        message: message.clone(),
                        arguments: call.arguments.clone(),
                    })
                    .await;
                if !approved {
                    return Err(DeepCoderError::ToolDenied { reason: message });
                }
            }
            PermissionResult::Allow => {}
        }

        tool.call(call.arguments.clone(), ctx).await
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}
