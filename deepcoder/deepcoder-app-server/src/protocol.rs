//! JSON-RPC 2.0 协议定义

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

pub type NotificationSender = mpsc::Sender<Value>;

/// JSON-RPC 请求
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC 响应
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 错误
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ThreadRecord {
    id: String,
    thread_id: String,
    model: String,
    message_count: u32,
}

/// In-memory JSON-RPC message processor.
pub struct MessageProcessor {
    config: deepcoder_config::Config,
    persistence: deepcoder_persistence::Persistence,
    threads: HashMap<String, ThreadRecord>,
    cancelled_turns: HashSet<String>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

impl MessageProcessor {
    pub fn new(config: deepcoder_config::Config) -> Self {
        let persistence = deepcoder_persistence::Persistence::new(config.system.data_dir.clone());
        let threads = persistence
            .list_sessions()
            .into_iter()
            .map(|entry| {
                (
                    entry.id.clone(),
                    ThreadRecord {
                        id: entry.id,
                        thread_id: entry.thread_id,
                        model: entry.model,
                        message_count: entry.message_count,
                    },
                )
            })
            .collect();
        Self {
            config,
            persistence,
            threads,
            cancelled_turns: HashSet::new(),
        }
    }

    pub async fn process_json(&mut self, input: &str) -> JsonRpcResponse {
        match serde_json::from_str::<JsonRpcRequest>(input) {
            Ok(request) => self.process(request).await,
            Err(error) => JsonRpcResponse::error(None, -32700, format!("Parse error: {error}")),
        }
    }

    pub async fn process_json_with_notifications(
        &mut self,
        input: &str,
        notifications: Option<NotificationSender>,
    ) -> JsonRpcResponse {
        match serde_json::from_str::<JsonRpcRequest>(input) {
            Ok(request) => {
                self.process_with_notifications(request, notifications)
                    .await
            }
            Err(error) => JsonRpcResponse::error(None, -32700, format!("Parse error: {error}")),
        }
    }

    pub async fn process(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        self.process_with_notifications(request, None).await
    }

    pub async fn process_with_notifications(
        &mut self,
        request: JsonRpcRequest,
        notifications: Option<NotificationSender>,
    ) -> JsonRpcResponse {
        let id = request.id.clone();
        if request.jsonrpc != "2.0" {
            return JsonRpcResponse::error(id, -32600, "Invalid Request: jsonrpc must be 2.0");
        }

        match request.method.as_str() {
            "initialize" => JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "server": "deepcoder-app-server",
                    "version": env!("CARGO_PKG_VERSION"),
                    "capabilities": {
                        "threads": true,
                        "turns": true,
                        "turnCancel": true
                    }
                }),
            ),
            "thread/create" => self.create_thread(id, request.params),
            "thread/list" => JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "threads": self.threads.values().collect::<Vec<_>>()
                }),
            ),
            "thread/get" => self.get_thread(id, request.params),
            "thread/delete" => self.delete_thread(id, request.params),
            "turn/start" => self.start_turn(id, request.params, notifications).await,
            "turn/cancel" => self.cancel_turn(id, request.params),
            _ => {
                JsonRpcResponse::error(id, -32601, format!("Method not found: {}", request.method))
            }
        }
    }

    fn create_thread(&mut self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let model = params
            .as_ref()
            .and_then(|value| value.get("model"))
            .and_then(|value| value.as_str())
            .unwrap_or(&self.config.provider.model)
            .to_string();
        let mut config = self.config.clone();
        config.provider.model = model;
        let router = Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let session = deepcoder_engine::Session::new(config, router);
        if let Err(error) = self
            .persistence
            .upsert_thread(&session.id, &session.thread, None)
        {
            return JsonRpcResponse::error(id, -32000, format!("Persistence error: {error}"));
        }
        let thread = thread_record_from_session(&session);
        self.threads.insert(thread.id.clone(), thread.clone());
        JsonRpcResponse::success(id, serde_json::json!({ "thread": thread }))
    }

    fn get_thread(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let Some(thread_id) = thread_id_param(params.as_ref()) else {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing thread_id");
        };
        match self.threads.get(thread_id) {
            Some(thread) => {
                let messages = match thread_id
                    .parse::<uuid::Uuid>()
                    .ok()
                    .map(|session_id| self.persistence.load_snapshot(&session_id))
                {
                    Some(Ok(Some(snapshot))) => snapshot.messages,
                    Some(Ok(None)) | None => Vec::new(),
                    Some(Err(error)) => {
                        return JsonRpcResponse::error(
                            id,
                            -32000,
                            format!("Persistence error: {error}"),
                        );
                    }
                };
                JsonRpcResponse::success(
                    id,
                    serde_json::json!({
                        "thread": thread,
                        "messages": messages
                    }),
                )
            }
            None => JsonRpcResponse::error(id, -32004, format!("Thread not found: {thread_id}")),
        }
    }

    fn delete_thread(&mut self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let Some(thread_id) = thread_id_param(params.as_ref()) else {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing thread_id");
        };
        let deleted = self.threads.remove(thread_id).is_some();
        if let Ok(session_id) = thread_id.parse::<uuid::Uuid>()
            && let Err(error) = self.persistence.delete_session(&session_id)
        {
            return JsonRpcResponse::error(id, -32000, format!("Persistence error: {error}"));
        }
        JsonRpcResponse::success(id, serde_json::json!({ "deleted": deleted }))
    }

    async fn start_turn(
        &mut self,
        id: Option<Value>,
        params: Option<Value>,
        notifications: Option<NotificationSender>,
    ) -> JsonRpcResponse {
        let Some(params) = params.as_ref() else {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing params");
        };
        let Some(session_id_text) = thread_id_param(Some(params)) else {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing thread_id");
        };
        let Ok(session_id) = session_id_text.parse::<uuid::Uuid>() else {
            return JsonRpcResponse::error(id, -32602, "Invalid params: thread_id must be UUID");
        };
        let Some(input) = params
            .get("input")
            .or_else(|| params.get("query"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
        else {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing input");
        };
        if let Some(turn_id) = params.get("turn_id").and_then(|value| value.as_str())
            && self.cancelled_turns.remove(turn_id)
        {
            return JsonRpcResponse::error(
                id,
                -32008,
                format!("Turn cancelled before start: {turn_id}"),
            );
        }

        let router = Arc::new(deepcoder_tools::ToolRouter::with_builtins());
        let mut session = match deepcoder_engine::Session::resume(
            self.config.clone(),
            router,
            &self.persistence,
            session_id,
        ) {
            Ok(Some(session)) => session,
            Ok(None) => {
                return JsonRpcResponse::error(
                    id,
                    -32004,
                    format!("Thread not found: {session_id_text}"),
                );
            }
            Err(error) => {
                return JsonRpcResponse::error(id, -32000, format!("Persistence error: {error}"));
            }
        };

        let (tx, mut rx) = broadcast::channel(1024);
        let mut events = Vec::new();
        let result = {
            let mut run_turn = Box::pin(deepcoder_engine::turn::run_turn(&mut session, &input, tx));
            loop {
                tokio::select! {
                    result = &mut run_turn => break result,
                    received = rx.recv() => {
                        match received {
                            Ok(event) => push_turn_event(&notifications, &mut events, event),
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                tracing::warn!("AppServer turn event stream lagged by {skipped} events");
                            }
                            Err(broadcast::error::RecvError::Closed) => {}
                        }
                    }
                }
            }
        };
        while let Ok(event) = rx.try_recv() {
            push_turn_event(&notifications, &mut events, event);
        }

        match result {
            Ok(turn) => {
                let thread = thread_record_from_session(&session);
                self.threads.insert(thread.id.clone(), thread.clone());
                JsonRpcResponse::success(
                    id,
                    serde_json::json!({
                        "turn_id": turn.turn_id,
                        "thread": thread,
                        "events": events,
                        "token_usage": turn.token_usage
                    }),
                )
            }
            Err(error) => JsonRpcResponse::error(id, -32000, format!("Turn error: {error}")),
        }
    }

    fn cancel_turn(&mut self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let Some(params) = params.as_ref() else {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing params");
        };
        let Some(turn_id) = params
            .get("turn_id")
            .or_else(|| params.get("turnId"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        else {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing turn_id");
        };
        let inserted = self.cancelled_turns.insert(turn_id.to_string());
        JsonRpcResponse::success(
            id,
            serde_json::json!({
                "turn_id": turn_id,
                "cancelled": true,
                "new": inserted
            }),
        )
    }
}

fn push_turn_event(
    notifications: &Option<NotificationSender>,
    events: &mut Vec<Value>,
    event: deepcoder_types::event::EngineEvent,
) {
    let event_json = engine_event_to_json(event);
    if let Some(notifications) = notifications {
        let _ = notifications.try_send(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "turn/event",
            "params": event_json.clone()
        }));
    }
    events.push(event_json);
}

fn thread_id_param(params: Option<&Value>) -> Option<&str> {
    let params = params?;
    params
        .get("thread_id")
        .or_else(|| params.get("session_id"))
        .or_else(|| params.get("threadId"))
        .or_else(|| params.get("sessionId"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
}

fn thread_record_from_session(session: &deepcoder_engine::Session) -> ThreadRecord {
    ThreadRecord {
        id: session.id.to_string(),
        thread_id: session.thread.id.to_string(),
        model: session.thread.model.clone(),
        message_count: session.thread.message_count,
    }
}

fn engine_event_to_json(event: deepcoder_types::event::EngineEvent) -> Value {
    match event {
        deepcoder_types::event::EngineEvent::TurnStart { thread_id, turn_id } => {
            serde_json::json!({"type": "turn_start", "thread_id": thread_id, "turn_id": turn_id})
        }
        deepcoder_types::event::EngineEvent::TextDelta { thread_id, content } => {
            serde_json::json!({"type": "text_delta", "thread_id": thread_id, "content": content})
        }
        deepcoder_types::event::EngineEvent::ReasoningDelta { thread_id, content } => {
            serde_json::json!({"type": "reasoning_delta", "thread_id": thread_id, "content": content})
        }
        deepcoder_types::event::EngineEvent::ToolCallRequested {
            thread_id,
            tool_call,
        } => {
            serde_json::json!({"type": "tool_call", "thread_id": thread_id, "tool_call": tool_call})
        }
        deepcoder_types::event::EngineEvent::ToolResult {
            thread_id,
            tool_call_id,
            result,
            is_error,
        } => serde_json::json!({
            "type": "tool_result",
            "thread_id": thread_id,
            "tool_call_id": tool_call_id,
            "result": result,
            "is_error": is_error
        }),
        deepcoder_types::event::EngineEvent::TurnComplete {
            thread_id,
            turn_id,
            token_usage,
        } => serde_json::json!({
            "type": "turn_complete",
            "thread_id": thread_id,
            "turn_id": turn_id,
            "token_usage": token_usage
        }),
        deepcoder_types::event::EngineEvent::Error { thread_id, message } => {
            serde_json::json!({"type": "error", "thread_id": thread_id, "message": message})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "deepcoder_app_server_{name}_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn config(name: &str) -> deepcoder_config::Config {
        let mut config = deepcoder_config::Config::load_default().unwrap();
        config.api_key = Some("test-key".into());
        config.system.data_dir = temp_dir(name);
        config
    }

    fn processor() -> MessageProcessor {
        MessageProcessor::new(config("processor"))
    }

    #[tokio::test]
    async fn parses_initialize_request() {
        let mut processor = processor();
        let response = processor
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
            .await;
        assert!(response.error.is_none());
        assert_eq!(response.result.unwrap()["server"], "deepcoder-app-server");
    }

    #[tokio::test]
    async fn unknown_method_returns_json_rpc_error() {
        let mut processor = processor();
        let response = processor
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"missing"}"#)
            .await;
        let error = response.error.unwrap();
        assert_eq!(error.code, -32601);
        assert!(error.message.contains("Method not found"));
    }

    #[tokio::test]
    async fn invalid_json_returns_parse_error() {
        let mut processor = processor();
        let response = processor.process_json("{not-json}").await;
        let error = response.error.unwrap();
        assert_eq!(error.code, -32700);
    }

    #[tokio::test]
    async fn thread_crud_round_trip() {
        let mut processor = processor();
        let created = processor.process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"thread/create","params":{"model":"test-model"}}"#,
        ).await;
        let thread_id = created.result.unwrap()["thread"]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let listed = processor
            .process_json(r#"{"jsonrpc":"2.0","id":2,"method":"thread/list"}"#)
            .await;
        assert_eq!(
            listed.result.unwrap()["threads"].as_array().unwrap().len(),
            1
        );

        let got = processor.process_json(&format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"thread/get","params":{{"thread_id":"{thread_id}"}}}}"#
        )).await;
        assert_eq!(got.result.unwrap()["thread"]["model"], "test-model");

        let deleted = processor.process_json(&format!(
            r#"{{"jsonrpc":"2.0","id":4,"method":"thread/delete","params":{{"thread_id":"{thread_id}"}}}}"#
        )).await;
        assert_eq!(deleted.result.unwrap()["deleted"], true);
    }

    #[tokio::test]
    async fn missing_thread_id_returns_invalid_params() {
        let mut processor = processor();
        let response = processor
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"thread/get"}"#)
            .await;
        let error = response.error.unwrap();
        assert_eq!(error.code, -32602);
    }

    #[tokio::test]
    async fn turn_cancel_records_cancel_request() {
        let mut processor = processor();
        let response = processor
            .process_json(
                r#"{"jsonrpc":"2.0","id":1,"method":"turn/cancel","params":{"turn_id":"turn-1"}}"#,
            )
            .await;
        let result = response.result.unwrap();
        assert_eq!(result["turn_id"], "turn-1");
        assert_eq!(result["cancelled"], true);

        let response = processor
            .process_json(r#"{"jsonrpc":"2.0","id":2,"method":"turn/cancel","params":{}}"#)
            .await;
        assert_eq!(response.error.unwrap().code, -32602);
    }

    #[tokio::test]
    async fn turn_start_streams_events() {
        let body = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"hello app\"}}]}\n\ndata: [DONE]\n\n";
        let (base_url, handle) =
            spawn_http_server(vec![http_response("200 OK", "text/event-stream", body)]).await;
        let mut config = config("turn_start");
        config.provider.base_url = base_url;
        let mut processor = MessageProcessor::new(config);
        let created = processor
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"thread/create"}"#)
            .await;
        let thread_id = created.result.unwrap()["thread"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "turn/start",
            "params": {
                "thread_id": thread_id,
                "input": "hello"
            }
        });

        let (notification_tx, mut notification_rx) = mpsc::channel(128);
        let response = processor
            .process_json_with_notifications(&request.to_string(), Some(notification_tx))
            .await;
        assert!(
            response.error.is_none(),
            "unexpected turn/start error: {:?}",
            response.error
        );
        let result = response.result.unwrap();
        let events = result["events"].as_array().unwrap();
        assert!(events.iter().any(|event| event["type"] == "text_delta"));
        assert!(
            events
                .iter()
                .any(|event| event["type"] == "reasoning_delta")
        );
        assert!(events.iter().any(|event| event["type"] == "turn_complete"));
        assert!(result["token_usage"]["output_tokens"].as_u64().unwrap() > 0);
        let mut notifications = Vec::new();
        while let Ok(notification) = notification_rx.try_recv() {
            notifications.push(notification);
        }
        assert!(notifications.iter().any(|notification| {
            notification["method"] == "turn/event" && notification["params"]["type"] == "text_delta"
        }));
        assert!(notifications.iter().any(|notification| {
            notification["method"] == "turn/event"
                && notification["params"]["type"] == "turn_complete"
        }));

        let requests = handle.await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /chat/completions"));
    }

    #[tokio::test]
    async fn thread_get_returns_persisted_messages() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"remembered answer\"}}]}\n\ndata: [DONE]\n\n";
        let (base_url, handle) =
            spawn_http_server(vec![http_response("200 OK", "text/event-stream", body)]).await;
        let mut config = config("thread_get_messages");
        config.provider.base_url = base_url;
        let mut processor = MessageProcessor::new(config);
        let created = processor
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"thread/create"}"#)
            .await;
        let thread_id = created.result.unwrap()["thread"]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let start = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "turn/start",
            "params": {
                "thread_id": thread_id,
                "input": "remember me"
            }
        });
        let response = processor.process_json(&start.to_string()).await;
        assert!(
            response.error.is_none(),
            "unexpected turn/start error: {:?}",
            response.error
        );

        let got = processor
            .process_json(&format!(
                r#"{{"jsonrpc":"2.0","id":3,"method":"thread/get","params":{{"thread_id":"{thread_id}"}}}}"#
            ))
            .await;
        let result = got.result.unwrap();
        let messages = result["messages"].as_array().unwrap();
        assert!(messages.iter().any(|message| {
            message["role"] == "User"
                && message["contents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|content| content["Text"].as_str() == Some("remember me"))
        }));
        assert!(messages.iter().any(|message| {
            message["role"] == "Assistant"
                && message["contents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|content| content["Text"].as_str() == Some("remembered answer"))
        }));

        let requests = handle.await.unwrap();
        assert_eq!(requests.len(), 1);
    }

    #[tokio::test]
    async fn turn_start_rejects_pre_cancelled_turn_id() {
        let mut processor = processor();
        let created = processor
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"thread/create"}"#)
            .await;
        let thread_id = created.result.unwrap()["thread"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        processor
            .process_json(
                r#"{"jsonrpc":"2.0","id":2,"method":"turn/cancel","params":{"turn_id":"turn-2"}}"#,
            )
            .await;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "turn/start",
            "params": {
                "thread_id": thread_id,
                "turn_id": "turn-2",
                "input": "hello"
            }
        });

        let response = processor.process_json(&request.to_string()).await;
        let error = response.error.unwrap();
        assert_eq!(error.code, -32008);
        assert!(error.message.contains("cancelled"));
    }

    fn http_response(status: &str, content_type: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    async fn spawn_http_server(responses: Vec<String>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                requests.push(read_http_request(&mut socket).await);
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });

        (format!("http://{addr}"), handle)
    }

    async fn read_http_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut buffer = Vec::new();
        let mut header_end = None;
        let mut content_length = 0usize;
        let mut chunk = [0u8; 4096];

        loop {
            let size = socket.read(&mut chunk).await.unwrap();
            if size == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..size]);

            if header_end.is_none()
                && let Some(position) = find_header_end(&buffer)
            {
                header_end = Some(position);
                let headers = String::from_utf8_lossy(&buffer[..position]);
                content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or_default();
            }

            if let Some(position) = header_end
                && buffer.len() >= position + 4 + content_length
            {
                break;
            }
        }

        String::from_utf8_lossy(&buffer).to_string()
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }
}
