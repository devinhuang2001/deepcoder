//! DeepCoder 工具系统
//!
//! 定义 Tool trait、ToolRouter、及内置工具实现。

pub mod file;
pub(crate) mod path;
pub mod router;
pub mod search;
pub mod shell;
pub mod task;
pub mod traits;
pub mod web;

pub use router::ToolRouter;
pub use traits::{Tool, ToolApprovalRequest, ToolApprover, ToolContext};

use std::sync::Arc;

pub fn builtin_tools() -> Vec<Arc<dyn Tool>> {
    let task_store = task::TaskStore::default();
    vec![
        Arc::new(file::FileReadTool),
        Arc::new(file::FileWriteTool),
        Arc::new(file::FileEditTool),
        Arc::new(shell::BashTool),
        Arc::new(search::GlobTool),
        Arc::new(search::GrepTool),
        Arc::new(web::WebFetchTool),
        Arc::new(web::WebSearchTool),
        Arc::new(task::AgentTool::new(task_store.clone())),
        Arc::new(task::TaskCreateTool::new(task_store.clone())),
        Arc::new(task::TaskListTool::new(task_store.clone())),
        Arc::new(task::TaskUpdateTool::new(task_store.clone())),
        Arc::new(task::TaskDeleteTool::new(task_store)),
    ]
}
