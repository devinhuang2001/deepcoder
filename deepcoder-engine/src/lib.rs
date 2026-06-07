//! DeepCoder 核心引擎
//!
//! 会话管理、回合循环、事件流。

pub mod session;
pub mod turn;

pub use session::Session;
pub use turn::run_turn;

use deepcoder_error::DeepCoderResult;

/// 批处理执行（简化入口）
pub async fn run_exec(config: &deepcoder_config::Config, query: &str) -> DeepCoderResult<String> {
    let tool_router = std::sync::Arc::new(deepcoder_tools::ToolRouter::new());
    let mut session = Session::new(config.clone(), tool_router.clone());
    let (tx, _rx) = tokio::sync::broadcast::channel(256);

    let result = turn::run_turn(&mut session, query, tx).await?;

    // 提取最后一条助手消息
    let last_msg = session.messages.iter()
        .rev()
        .find(|m| m.role == deepcoder_types::message::MessageRole::Assistant)
        .and_then(|m| {
            m.contents.iter()
                .find_map(|c| match c {
                    deepcoder_types::message::ContentType::Text(t) => Some(t.clone()),
                    _ => None,
                })
        })
        .unwrap_or_default();

    Ok(last_msg)
}
