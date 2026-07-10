//! 回合循环 — 核心 Agent Loop

use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_persistence::Persistence;
use deepcoder_types::event::*;
use deepcoder_types::message::*;
use deepcoder_types::provider::*;
use deepcoder_types::session::*;
use deepcoder_types::tool::*;
use tokio::sync::broadcast;

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
    let provider = create_provider(&session.config)?;
    run_turn_with_provider(session, user_input, event_tx, provider.as_ref()).await
}

async fn run_turn_with_provider(
    session: &mut Session,
    user_input: &str,
    event_tx: broadcast::Sender<EngineEvent>,
    provider: &dyn deepcoder_provider::ModelProvider,
) -> DeepCoderResult<TurnResult> {
    let turn_id = uuid::Uuid::new_v4();
    let thread_id = session.thread.id;
    let persistence = Persistence::new(session.config.system.data_dir.clone());

    // 1. 添加用户消息
    add_persisted_message(
        session,
        &persistence,
        Message::text(MessageRole::User, user_input),
    );

    // 2. 发送回合开始事件
    let _ = event_tx.send(EngineEvent::TurnStart { thread_id, turn_id });
    persist_event(
        &persistence,
        &session.id,
        "turn_start",
        serde_json::json!({
            "thread_id": thread_id,
            "turn_id": turn_id
        }),
    );

    // 主循环：模型响应 → 工具调用 → 结果反馈 → 继续
    let mut tool_iterations = 0u32;
    let max_iter = session.config.system.max_tool_iterations;
    let mut turn_usage = TokenUsage::default();

    loop {
        let request = build_chat_request(session).await?;
        turn_usage.input_tokens += estimate_request_tokens(&request);
        let mut stream = provider.chat_stream(request).await?;
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut assistant_content = String::new();
        let mut reasoning_content = String::new();

        // 4a. 读取流事件
        while let Some(event) = stream.next_event().await {
            match event? {
                StreamEvent::TextDelta(text) => {
                    assistant_content.push_str(&text);
                    let _ = event_tx.send(EngineEvent::TextDelta {
                        thread_id,
                        content: text.clone(),
                    });
                    persist_event(
                        &persistence,
                        &session.id,
                        "text_delta",
                        serde_json::json!({
                            "thread_id": thread_id,
                            "content": text
                        }),
                    );
                }
                StreamEvent::ReasoningDelta(text) => {
                    reasoning_content.push_str(&text);
                    let _ = event_tx.send(EngineEvent::ReasoningDelta {
                        thread_id,
                        content: text.clone(),
                    });
                    persist_event(
                        &persistence,
                        &session.id,
                        "reasoning_delta",
                        serde_json::json!({
                            "thread_id": thread_id,
                            "content": text
                        }),
                    );
                }
                StreamEvent::ToolCall {
                    id,
                    name,
                    arguments,
                } => {
                    let tool_call = ToolCall {
                        call_id: id,
                        tool_name: name,
                        arguments,
                    };
                    let _ = event_tx.send(EngineEvent::ToolCallRequested {
                        thread_id,
                        tool_call: tool_call.clone(),
                    });
                    persist_event(
                        &persistence,
                        &session.id,
                        "tool_call",
                        serde_json::json!({
                            "thread_id": thread_id,
                            "tool_call": tool_call.clone()
                        }),
                    );
                    tool_calls.push(tool_call);
                }
                StreamEvent::Done => break,
                StreamEvent::Error(e) => {
                    let _ = event_tx.send(EngineEvent::Error {
                        thread_id,
                        message: e.clone(),
                    });
                    persist_event(
                        &persistence,
                        &session.id,
                        "error",
                        serde_json::json!({
                            "thread_id": thread_id,
                            "message": e
                        }),
                    );
                    return Err(DeepCoderError::Provider(e));
                }
            }
        }

        // 4b. 添加助手消息（含推理内容）
        let mut contents = Vec::new();
        if !reasoning_content.is_empty() {
            contents.push(ContentType::Reasoning {
                content: reasoning_content.clone(),
            });
            add_persisted_message(
                session,
                &persistence,
                Message::reasoning(MessageRole::Assistant, &reasoning_content),
            );
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
            add_persisted_message(
                session,
                &persistence,
                Message::new(MessageRole::Assistant, contents),
            );
        }
        turn_usage.output_tokens += estimate_text_tokens(&assistant_content);
        turn_usage.reasoning_tokens += estimate_text_tokens(&reasoning_content);

        // 4c. 处理工具调用
        if tool_calls.is_empty() {
            break; // 无工具调用，回合结束
        }

        tool_iterations += 1;
        if tool_iterations > max_iter {
            return Err(DeepCoderError::Turn(format!(
                "超过最大工具迭代次数 ({max_iter})"
            )));
        }

        // 4d. 执行工具
        let ctx = deepcoder_tools::ToolContext {
            config: session.config.clone(),
            workspace_root: std::env::current_dir().ok(),
            tool_router: Some(session.tool_router.clone()),
            tool_approver: session.tool_approver.clone(),
        };

        let tool_results = execute_tool_calls(session.tool_router.clone(), &tool_calls, ctx).await;
        for (tc, output) in tool_results {
            let (result, is_error) = match output {
                Ok(output) => (output.as_json(), false),
                Err(e) => (serde_json::json!({ "error": e.to_string() }), true),
            };
            add_persisted_message(
                session,
                &persistence,
                Message::new(
                    MessageRole::Tool,
                    vec![ContentType::ToolResult {
                        id: tc.call_id.clone(),
                        content: result.clone(),
                        is_error,
                    }],
                ),
            );
            let _ = event_tx.send(EngineEvent::ToolResult {
                thread_id,
                tool_call_id: tc.call_id.clone(),
                result: result.clone(),
                is_error,
            });
            persist_event(
                &persistence,
                &session.id,
                "tool_result",
                serde_json::json!({
                    "thread_id": thread_id,
                    "tool_call_id": tc.call_id.clone(),
                    "result": result,
                    "is_error": is_error
                }),
            );
        }

        // 4e. 工具结果已加入消息历史，下一轮重新构建请求并继续。
    }

    // 5. 回合完成
    session.thread.token_usage.add(&turn_usage);
    let _ = event_tx.send(EngineEvent::TurnComplete {
        thread_id,
        turn_id,
        token_usage: session.thread.token_usage.clone(),
    });
    persist_event(
        &persistence,
        &session.id,
        "turn_complete",
        serde_json::json!({
            "thread_id": thread_id,
            "turn_id": turn_id,
            "token_usage": session.thread.token_usage
        }),
    );
    if let Err(error) = persistence.upsert_thread(&session.id, &session.thread, None) {
        tracing::warn!("failed to update session index: {error}");
    }

    Ok(TurnResult {
        turn_id,
        messages: session.messages.clone(),
        token_usage: session.thread.token_usage.clone(),
    })
}

async fn build_chat_request(session: &Session) -> DeepCoderResult<ChatRequest> {
    let mut messages = build_chat_messages(&session.messages);
    if let Some(skill_prompt) = build_skill_system_prompt(&session.config) {
        messages.insert(
            0,
            ChatMessage {
                role: "system".into(),
                content: serde_json::Value::String(skill_prompt),
                tool_call_id: None,
                tool_calls: None,
            },
        );
    }
    messages = crate::context::compact_chat_messages(
        messages,
        session.config.system.max_context_tokens as u64,
        12,
    )?;

    Ok(ChatRequest {
        model: session.config.provider.model.clone(),
        messages,
        tools: session.tool_router.direct_specs().await,
        stream: true,
        max_tokens: session.config.provider.max_tokens,
        temperature: session.config.provider.temperature,
    })
}

fn build_skill_system_prompt(config: &deepcoder_config::Config) -> Option<String> {
    let mut manager = deepcoder_skills::SkillsManager::new();
    if let Err(error) = manager.load_from_dir(std::path::Path::new(".deepcoder/skills")) {
        tracing::warn!("failed to load project skills: {error}");
    }
    let data_skills_dir = config.system.data_dir.join("skills");
    if let Err(error) = manager.load_from_dir(&data_skills_dir) {
        tracing::warn!("failed to load data-dir skills: {error}");
    }
    let skills = manager.get_active_skills(&[]);
    let prompt = deepcoder_skills::SkillsManager::build_prompt_injection_for(&skills);
    (!prompt.trim().is_empty()).then_some(prompt)
}

fn build_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages.iter().flat_map(message_to_chat_messages).collect()
}

fn estimate_request_tokens(request: &ChatRequest) -> u64 {
    let message_tokens = request
        .messages
        .iter()
        .map(|message| {
            estimate_value_tokens(&message.content)
                + message
                    .tool_calls
                    .as_ref()
                    .map(|tool_calls| {
                        estimate_text_tokens(&serde_json::to_string(tool_calls).unwrap_or_default())
                    })
                    .unwrap_or_default()
        })
        .sum::<u64>();
    let tool_tokens =
        estimate_text_tokens(&serde_json::to_string(&request.tools).unwrap_or_default());
    message_tokens + tool_tokens
}

fn estimate_value_tokens(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::String(text) => estimate_text_tokens(text),
        serde_json::Value::Null => 0,
        other => estimate_text_tokens(&other.to_string()),
    }
}

fn estimate_text_tokens(text: &str) -> u64 {
    let chars = text.chars().filter(|ch| !ch.is_whitespace()).count() as u64;
    if chars == 0 {
        0
    } else {
        chars.div_ceil(4).max(1)
    }
}

fn add_persisted_message(session: &mut Session, persistence: &Persistence, message: Message) {
    session.add_message(message.clone());
    if let Err(error) = persistence.record_message(&session.id, &session.thread, &message) {
        tracing::warn!("failed to persist message: {error}");
    }
}

fn persist_event(
    persistence: &Persistence,
    session_id: &uuid::Uuid,
    event_type: &str,
    payload: serde_json::Value,
) {
    if let Err(error) = persistence.append_session_event(session_id, event_type, payload) {
        tracing::warn!("failed to persist {event_type} event: {error}");
    }
}

fn message_to_chat_messages(message: &Message) -> Vec<ChatMessage> {
    match message.role {
        MessageRole::System | MessageRole::User => {
            let text = text_content(&message.contents);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![ChatMessage {
                    role: role_name(&message.role).into(),
                    content: serde_json::Value::String(text),
                    tool_call_id: None,
                    tool_calls: None,
                }]
            }
        }
        MessageRole::Assistant => assistant_chat_message(&message.contents)
            .map(|msg| vec![msg])
            .unwrap_or_default(),
        MessageRole::Tool => message
            .contents
            .iter()
            .filter_map(|content| match content {
                ContentType::ToolResult { id, content, .. } => Some(ChatMessage {
                    role: "tool".into(),
                    content: serde_json::Value::String(tool_result_content(content)),
                    tool_call_id: Some(id.clone()),
                    tool_calls: None,
                }),
                _ => None,
            })
            .collect(),
    }
}

fn assistant_chat_message(contents: &[ContentType]) -> Option<ChatMessage> {
    let text = text_content(contents);
    let tool_calls: Vec<_> = contents
        .iter()
        .filter_map(|content| match content {
            ContentType::ToolCall {
                id,
                name,
                arguments,
            } => Some(serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": arguments.to_string()
                }
            })),
            _ => None,
        })
        .collect();

    if text.is_empty() && tool_calls.is_empty() {
        return None;
    }

    Some(ChatMessage {
        role: "assistant".into(),
        content: if text.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(text)
        },
        tool_call_id: None,
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
    })
}

async fn execute_tool_calls(
    router: std::sync::Arc<deepcoder_tools::ToolRouter>,
    tool_calls: &[ToolCall],
    ctx: deepcoder_tools::ToolContext,
) -> Vec<(ToolCall, DeepCoderResult<JsonToolOutput>)> {
    if are_all_tools_concurrency_safe(router.as_ref(), tool_calls).await {
        futures::future::join_all(tool_calls.iter().cloned().map(|tc| {
            let router = router.clone();
            let ctx = ctx.clone();
            async move {
                let result = router.execute(&tc, &ctx).await;
                (tc, result)
            }
        }))
        .await
    } else {
        let mut results = Vec::with_capacity(tool_calls.len());
        for tc in tool_calls {
            results.push((tc.clone(), router.execute(tc, &ctx).await));
        }
        results
    }
}

async fn are_all_tools_concurrency_safe(
    router: &deepcoder_tools::ToolRouter,
    tool_calls: &[ToolCall],
) -> bool {
    if tool_calls.len() <= 1 {
        return false;
    }
    for tc in tool_calls {
        if !router.is_concurrency_safe(&tc.tool_name).await {
            return false;
        }
    }
    true
}

fn text_content(contents: &[ContentType]) -> String {
    contents
        .iter()
        .filter_map(|content| match content {
            ContentType::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_result_content(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn role_name(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

/// 创建 Provider 辅助函数
fn create_provider(
    config: &deepcoder_config::Config,
) -> DeepCoderResult<Box<dyn deepcoder_provider::ModelProvider>> {
    let api_key = config
        .api_key
        .clone()
        .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
        .ok_or_else(|| DeepCoderError::Config("DEEPSEEK_API_KEY 未设置".into()))?;

    Ok(Box::new(
        deepcoder_provider::deepseek::DeepSeekProvider::new(
            api_key,
            config.provider.model.clone(),
            config.provider.base_url.clone(),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepcoder_provider::{ModelProvider, StreamReceiver};
    use deepcoder_tools::{Tool, ToolContext, ToolRouter};
    use deepcoder_types::provider::{ProviderCapabilities, ProviderInfo, StreamEvent};
    use std::collections::VecDeque;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("deepcoder_engine_{name}_{}", std::process::id()));
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_config(name: &str) -> deepcoder_config::Config {
        let mut config = deepcoder_config::Config::load_default().unwrap();
        config.api_key = Some("test-key".into());
        config.system.data_dir = temp_dir(name);
        config
    }

    fn persisted_event_types(data_dir: std::path::PathBuf, session_id: uuid::Uuid) -> Vec<String> {
        let persistence = Persistence::new(data_dir);
        let mut event_types = Vec::new();
        for path in persistence.session_log_paths(&session_id).unwrap() {
            for event in deepcoder_persistence::jsonl::read_events(&path).unwrap() {
                if let Some(event_type) = event.get("type").and_then(|value| value.as_str()) {
                    event_types.push(event_type.to_string());
                }
            }
        }
        event_types
    }

    #[test]
    fn chat_messages_convert_assistant_tool_calls_and_results() {
        let messages = vec![
            Message::text(MessageRole::User, "please read"),
            Message::new(
                MessageRole::Assistant,
                vec![ContentType::ToolCall {
                    id: "call_1".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path": "README.md"}),
                }],
            ),
            Message::new(
                MessageRole::Tool,
                vec![ContentType::ToolResult {
                    id: "call_1".into(),
                    content: serde_json::json!({"content": "hello"}),
                    is_error: false,
                }],
            ),
        ];

        let chat = build_chat_messages(&messages);
        assert_eq!(chat.len(), 3);
        assert_eq!(chat[1].role, "assistant");
        assert_eq!(chat[1].content, serde_json::Value::Null);
        assert_eq!(
            chat[1].tool_calls.as_ref().unwrap()[0]["function"]["name"],
            "read_file"
        );
        assert_eq!(chat[2].role, "tool");
        assert_eq!(chat[2].tool_call_id.as_deref(), Some("call_1"));
        assert!(chat[2].content.as_str().unwrap().contains("hello"));
    }

    #[test]
    fn chat_messages_exclude_reasoning_only_messages() {
        let messages = vec![
            Message::reasoning(MessageRole::Assistant, "private reasoning"),
            Message::text(MessageRole::Assistant, "final answer"),
        ];

        let chat = build_chat_messages(&messages);
        assert_eq!(chat.len(), 1);
        assert_eq!(chat[0].content, "final answer");
    }

    #[tokio::test]
    async fn engine_records_turn_events() {
        let config = test_config("turn_events");
        let data_dir = config.system.data_dir.clone();
        let router = std::sync::Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let mut session = Session::new(config, router);
        let session_id = session.id;
        let provider = TextProvider::new("hello from provider");
        let (tx, _rx) = broadcast::channel(32);

        run_turn_with_provider(&mut session, "say hello", tx, &provider)
            .await
            .unwrap();

        let event_types = persisted_event_types(data_dir.clone(), session_id);
        assert!(
            event_types
                .iter()
                .any(|event_type| event_type == "turn_start")
        );
        assert!(event_types.iter().any(|event_type| event_type == "message"));
        assert!(
            event_types
                .iter()
                .any(|event_type| event_type == "text_delta")
        );
        assert!(
            event_types
                .iter()
                .any(|event_type| event_type == "turn_complete")
        );

        let snapshot = Persistence::new(data_dir)
            .load_snapshot(&session_id)
            .unwrap()
            .unwrap();
        assert!(snapshot.messages.iter().any(|message| {
            message.contents.iter().any(|content| match content {
                ContentType::Text(text) => text.contains("hello from provider"),
                _ => false,
            })
        }));
    }

    #[tokio::test]
    async fn token_usage_accumulates_per_turn() {
        let config = test_config("token_usage");
        let router = std::sync::Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let mut session = Session::new(config, router);
        let provider = TextProvider::new("first answer");
        let (tx, _rx) = broadcast::channel(32);

        run_turn_with_provider(&mut session, "first question", tx.clone(), &provider)
            .await
            .unwrap();
        let first_usage = session.thread.token_usage.clone();
        assert!(first_usage.input_tokens > 0);
        assert!(first_usage.output_tokens > 0);

        run_turn_with_provider(&mut session, "second question", tx, &provider)
            .await
            .unwrap();
        assert!(session.thread.token_usage.input_tokens > first_usage.input_tokens);
        assert!(session.thread.token_usage.output_tokens > first_usage.output_tokens);
    }

    #[tokio::test]
    async fn engine_injects_active_skills() {
        let config = test_config("skills");
        let skill_dir = config.system.data_dir.join("skills").join("rust");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: rust
description: Rust guidance
---
Always run cargo fmt.
"#,
        )
        .unwrap();
        let router = std::sync::Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let mut session = Session::new(config, router);
        let provider = TextProvider::new("ok");
        let (tx, _rx) = broadcast::channel(32);

        run_turn_with_provider(&mut session, "use skills", tx, &provider)
            .await
            .unwrap();

        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests[0].messages[0].role, "system");
        assert!(
            requests[0].messages[0]
                .content
                .as_str()
                .unwrap()
                .contains("Always run cargo fmt.")
        );
    }

    #[tokio::test]
    async fn tool_loop_sends_tool_result_back_to_provider() {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "deepcoder_engine_tool_loop_file_{}",
                std::process::id()
            ));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).ok();
        }
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("note.txt");
        std::fs::write(&file, "tool output").unwrap();

        let config = test_config("tool_loop");
        let data_dir = config.system.data_dir.clone();
        let router = std::sync::Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let mut session = Session::new(config, router);
        let session_id = session.id;
        let provider = MockProvider::new(file.display().to_string());
        let (tx, _rx) = broadcast::channel(32);

        run_turn_with_provider(&mut session, "read the file", tx, &provider)
            .await
            .unwrap();

        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].messages.iter().any(|message| {
            message.role == "tool"
                && message
                    .content
                    .as_str()
                    .is_some_and(|content| content.contains("tool output"))
        }));
        let final_text = session
            .messages
            .iter()
            .rev()
            .find_map(|message| {
                message.contents.iter().find_map(|content| match content {
                    ContentType::Text(text) => Some(text.as_str()),
                    _ => None,
                })
            })
            .unwrap();
        assert_eq!(final_text, "done after tool");

        let event_types = persisted_event_types(data_dir, session_id);
        assert!(
            event_types
                .iter()
                .any(|event_type| event_type == "tool_call")
        );
        assert!(
            event_types
                .iter()
                .any(|event_type| event_type == "tool_result")
        );
    }

    #[tokio::test]
    async fn safe_tool_calls_execute_concurrently() {
        let max_running = run_probe_tool_turn(true).await;
        assert_eq!(max_running, 2);
    }

    #[tokio::test]
    async fn unsafe_tool_calls_execute_sequentially() {
        let max_running = run_probe_tool_turn(false).await;
        assert_eq!(max_running, 1);
    }

    async fn run_probe_tool_turn(safe: bool) -> usize {
        let running = Arc::new(AtomicUsize::new(0));
        let max_running = Arc::new(AtomicUsize::new(0));
        let router = Arc::new(ToolRouter::new());
        router
            .register(Arc::new(ConcurrencyProbeTool {
                safe,
                running: running.clone(),
                max_running: max_running.clone(),
            }))
            .await;

        let config = test_config(if safe { "probe_safe" } else { "probe_unsafe" });
        let mut session = Session::new(config, router);
        let provider = MultiToolProvider::new("probe");
        let (tx, _rx) = broadcast::channel(32);

        run_turn_with_provider(&mut session, "run probes", tx, &provider)
            .await
            .unwrap();

        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[1]
                .messages
                .iter()
                .filter(|message| message.role == "tool")
                .count(),
            2
        );
        max_running.load(Ordering::SeqCst)
    }

    struct ConcurrencyProbeTool {
        safe: bool,
        running: Arc<AtomicUsize>,
        max_running: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl Tool for ConcurrencyProbeTool {
        fn name(&self) -> &'static str {
            "probe"
        }

        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: self.name().into(),
                description: "Probe tool concurrency.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            }
        }

        fn is_concurrency_safe(&self) -> bool {
            self.safe
        }

        async fn call(
            &self,
            _params: serde_json::Value,
            _ctx: &ToolContext,
        ) -> DeepCoderResult<JsonToolOutput> {
            let current = self.running.fetch_add(1, Ordering::SeqCst) + 1;
            update_max(&self.max_running, current);
            tokio::time::sleep(Duration::from_millis(40)).await;
            self.running.fetch_sub(1, Ordering::SeqCst);
            Ok(JsonToolOutput::success(serde_json::json!({
                "running_at_start": current
            })))
        }
    }

    fn update_max(max_running: &AtomicUsize, current: usize) {
        let mut observed = max_running.load(Ordering::SeqCst);
        while current > observed {
            match max_running.compare_exchange(
                observed,
                current,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(next) => observed = next,
            }
        }
    }

    struct TextProvider {
        info: ProviderInfo,
        text: String,
        requests: Mutex<Vec<ChatRequest>>,
    }

    impl TextProvider {
        fn new(text: impl Into<String>) -> Self {
            Self {
                info: ProviderInfo {
                    name: "mock".into(),
                    base_url: "mock://provider".into(),
                    model: "mock-model".into(),
                    capabilities: ProviderCapabilities::default(),
                },
                text: text.into(),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl ModelProvider for TextProvider {
        fn info(&self) -> &ProviderInfo {
            &self.info
        }

        fn capabilities(&self) -> &ProviderCapabilities {
            &self.info.capabilities
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> deepcoder_error::DeepCoderResult<Box<dyn StreamReceiver>> {
            self.requests.lock().unwrap().push(request);
            Ok(Box::new(MockStream {
                events: VecDeque::from(vec![
                    Ok(StreamEvent::TextDelta(self.text.clone())),
                    Ok(StreamEvent::Done),
                ]),
            }))
        }
    }

    struct MockProvider {
        info: ProviderInfo,
        requests: Mutex<Vec<ChatRequest>>,
        file_path: String,
    }

    impl MockProvider {
        fn new(file_path: String) -> Self {
            Self {
                info: ProviderInfo {
                    name: "mock".into(),
                    base_url: "mock://provider".into(),
                    model: "mock-model".into(),
                    capabilities: ProviderCapabilities::default(),
                },
                requests: Mutex::new(Vec::new()),
                file_path,
            }
        }
    }

    #[async_trait::async_trait]
    impl ModelProvider for MockProvider {
        fn info(&self) -> &ProviderInfo {
            &self.info
        }

        fn capabilities(&self) -> &ProviderCapabilities {
            &self.info.capabilities
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> deepcoder_error::DeepCoderResult<Box<dyn StreamReceiver>> {
            let mut requests = self.requests.lock().unwrap();
            let call_index = requests.len();
            requests.push(request);
            drop(requests);

            let events = if call_index == 0 {
                vec![
                    Ok(StreamEvent::ToolCall {
                        id: "call_read".into(),
                        name: "read_file".into(),
                        arguments: serde_json::json!({"path": self.file_path}),
                    }),
                    Ok(StreamEvent::Done),
                ]
            } else {
                vec![
                    Ok(StreamEvent::TextDelta("done after tool".into())),
                    Ok(StreamEvent::Done),
                ]
            };

            Ok(Box::new(MockStream {
                events: VecDeque::from(events),
            }))
        }
    }

    struct MultiToolProvider {
        info: ProviderInfo,
        requests: Mutex<Vec<ChatRequest>>,
        tool_name: &'static str,
    }

    impl MultiToolProvider {
        fn new(tool_name: &'static str) -> Self {
            Self {
                info: ProviderInfo {
                    name: "mock".into(),
                    base_url: "mock://provider".into(),
                    model: "mock-model".into(),
                    capabilities: ProviderCapabilities::default(),
                },
                requests: Mutex::new(Vec::new()),
                tool_name,
            }
        }
    }

    #[async_trait::async_trait]
    impl ModelProvider for MultiToolProvider {
        fn info(&self) -> &ProviderInfo {
            &self.info
        }

        fn capabilities(&self) -> &ProviderCapabilities {
            &self.info.capabilities
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> deepcoder_error::DeepCoderResult<Box<dyn StreamReceiver>> {
            let mut requests = self.requests.lock().unwrap();
            let call_index = requests.len();
            requests.push(request);
            drop(requests);

            let events = if call_index == 0 {
                vec![
                    Ok(StreamEvent::ToolCall {
                        id: "call_1".into(),
                        name: self.tool_name.into(),
                        arguments: serde_json::json!({}),
                    }),
                    Ok(StreamEvent::ToolCall {
                        id: "call_2".into(),
                        name: self.tool_name.into(),
                        arguments: serde_json::json!({}),
                    }),
                    Ok(StreamEvent::Done),
                ]
            } else {
                vec![
                    Ok(StreamEvent::TextDelta("done".into())),
                    Ok(StreamEvent::Done),
                ]
            };

            Ok(Box::new(MockStream {
                events: VecDeque::from(events),
            }))
        }
    }

    struct MockStream {
        events: VecDeque<deepcoder_error::DeepCoderResult<StreamEvent>>,
    }

    #[async_trait::async_trait]
    impl StreamReceiver for MockStream {
        async fn next_event(&mut self) -> Option<deepcoder_error::DeepCoderResult<StreamEvent>> {
            self.events.pop_front()
        }
    }
}
