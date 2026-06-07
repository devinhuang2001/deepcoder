//! DeepCoder 工具系统
//!
//! 定义 Tool trait、ToolRouter、及内置工具实现。

pub mod router;
pub mod traits;
pub mod file;
pub mod shell;
pub mod search;
pub mod web;

pub use traits::Tool;
pub use router::ToolRouter;
