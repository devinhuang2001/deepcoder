//! 回合循环 — 核心 Agent Loop

use std::sync::Arc;
use tokio::sync::broadcast;
use deepcoder_types::message::*;
use deepcoder_types::provider::*;
use deepcoder_types::session::*;
use deepcoder_types::tool::*;
use deepcoder_types::event::*;
use deepcoder_error::{DeepCoderResult, DeepCoderError};

use crate::session::Session;

/// 回合运行结果
pub struct TurnResult {
    pub turn_id: uuid::Uuid,
    pub messages: Vec<Message>,
    pub token_usage: TokenUsage,
}

/// 执行一个回合
pub async fn run_turn(
    session: &mut Session,
    user_input: &str,
    event_tx: broadcast::Sender<EngineEvent>,
) -> DeepCoderResult<TurnResult> {
    let turn_id = uuid::Uuid::new_v4();
    let thread_id = session.thread.id;

    // 1. 添加用户消息
    session.add_message(Message::text(MessageRole::User, user_input));

    // 2. 发送回合开始事件
    let _ = event_tx.send(EngineEvent::TurnStart { thread_id, turn_id });

    // 3. 构建 Provider 请求
    let provider = create_provider(&session.config)?;
    let tool_specs = session.tool_router.direct_specs().await;

    let chat_messages: Vec<ChatMessage> = session.messages.iter()
        .filter(|m| m.role != MessageRole::Reasoning) // 不发送推理历史
        .map(|m| ChatMessage {
            role: match m.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
                MessageRole::System => "system",
                _ => "user",
            }.to_string(),
            content: serde_json::to_value(&m.contents).unwrap_or_default(),
        })
        .collect();

    let request = ChatRequest {
        model: session.config.provider.model.clone(),
        messages: chat_messages,
        tools: tool_specs,
        stream: true,
        max_tokens: session.config.provider.max_tokens,
        temperature: session.config.provider.temperature,
    };

    // 4. 发送请求并处理流
    let mut stream = provider.chat_stream(request).await?;
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut assistant_content = String::new();
    let mut reasoning_content = String::new();

    // 主循环：模型响应 → 工具调用 → 结果反馈 → 继续
    let mut tool_iterations = 0u32;
    let max_iter = session.config.system.max_tool_iterations;

    loop {
        // 4a. 读取流事件
        while let Some(event) = stream.next_event().await {
            match event? {
                StreamEvent::TextDelta(text) => {
                    assistant_content.push_str(&text);
                    let _ = event_tx.send(EngineEvent::TextDelta { thread_id, content: text });
                }
                StreamEvent::ReasoningDelta(text) => {
                    reasoning_content.push_str(&text);
                    let _ = event_tx.send(EngineEvent::ReasoningDelta { thread_id, content: text });
                }
                StreamEvent::ToolCall { id, name, arguments } => {
                    let tc = ToolCall { call_id: id.clone(), tool_name: name.clone(), arguments: arguments.clone() };
                    tool_calls.push(tc);
                    let _ = event_tx.send(EngineEvent::ToolCallRequested {
                        thread_id,
                        tool_call: ToolCall { call_id: id, tool_name: name, arguments },
                    });
                }
                StreamEvent::Done => break,
                StreamEvent::Error(e) => {
                    let err_msg = e.clone();
                    let _ = event_tx.send(EngineEvent::Error { thread_id, message: e });
                    return Err(DeepCoderError::Provider(err_msg));
                }
            }
        }

        // 4b. 添加助手消息（含推理内容）
        let mut contents = Vec::new();
        if !reasoning_content.is_empty() {
            contents.push(ContentType::Reasoning { content: reasoning_content.clone() });
            session.add_message(Message::reasoning(MessageRole::Assistant, &reasoning_content));
        }
        if !assistant_content.is_empty() {
            contents.push(ContentType::Text(assistant_content.clone()));
        }
        for tc in &tool_calls {
            contents.push(ContentType::ToolCall {
                id: tc.call_id.clone(),
                name: tc.tool_name.clone(),
                arguments: tc.arguments.clone(),
            });
        }
        if !contents.is_empty() {
            session.add_message(Message::new(MessageRole::Assistant, contents));
        }

        // 4c. 处理工具调用
        if tool_calls.is_empty() {
            break; // 无工具调用，回合结束
        }

        tool_iterations += 1;
        if tool_iterations > max_iter {
            return Err(DeepCoderError::Turn(format!("超过最大工具迭代次数 ({max_iter})")));
        }

        // 4d. 执行工具
        let ctx = deepcoder_tools::ToolContext {
            config: session.config.clone(),
            tool_router: Some(session.tool_router.clone()),
        };

        for tc in &tool_calls {
            match session.tool_router.execute(tc, &ctx).await {
                Ok(output) => {
                    let result = output.as_json();
                    session.add_message(Message::new(MessageRole::Tool, vec![
                        ContentType::ToolResult {
                            id: tc.call_id.clone(),
                            content: result.clone(),
                            is_error: false,
                        }
                    ]));
                    let _ = event_tx.send(EngineEvent::ToolResult {
                        thread_id,
                        tool_call_id: tc.call_id.clone(),
                        result,
                        is_error: false,
                    });
                }
                Err(e) => {
                    session.add_message(Message::new(MessageRole::Tool, vec![
                        ContentType::ToolResult {
                            id: tc.call_id.clone(),
                            content: serde_json::json!({ "error": e.to_string() }),
                            is_error: true,
                        }
                    ]));
                    let _ = event_tx.send(EngineEvent::ToolResult {
                        thread_id,
                        tool_call_id: tc.call_id.clone(),
                        result: serde_json::json!({ "error": e.to_string() }),
                        is_error: true,
                    });
                }
            }
        }

        // 4e. 为下一轮迭代重置
        tool_calls.clear();
        assistant_content.clear();
        reasoning_content.clear();
    }

    // 5. 回合完成
    let _ = event_tx.send(EngineEvent::TurnComplete {
        thread_id,
        turn_id,
        token_usage: session.thread.token_usage.clone(),
    });

    Ok(TurnResult {
        turn_id,
        messages: session.messages.clone(),
        token_usage: session.thread.token_usage.clone(),
    })
}

/// 创建 Provider 辅助函数
fn create_provider(config: &deepcoder_config::Config) -> DeepCoderResult<Box<dyn deepcoder_provider::ModelProvider>> {
    let api_key = config.api_key.clone()
        .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
        .ok_or_else(|| DeepCoderError::Config("DEEPSEEK_API_KEY 未设置".into()))?;

    Ok(Box::new(deepcoder_provider::deepseek::DeepSeekProvider::new(
        api_key,
        config.provider.model.clone(),
        config.provider.base_url.clone(),
    )))
}
