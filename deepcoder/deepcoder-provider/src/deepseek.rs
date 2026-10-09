//! DeepSeek Provider implementation.

use async_trait::async_trait;
use bytes::Bytes;
use futures::{StreamExt, stream::BoxStream};
use reqwest::{Client, StatusCode};
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_types::provider::*;

use super::{ModelProvider, StreamReceiver};

/// DeepSeek V4 Provider.
pub struct DeepSeekProvider {
    client: Client,
    api_key: String,
    info: ProviderInfo,
    capabilities: ProviderCapabilities,
    max_retries: usize,
    retry_base_delay: Duration,
}

impl DeepSeekProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        let capabilities = ProviderCapabilities::default();
        let client = build_http_client(&base_url);
        Self {
            client,
            api_key,
            info: ProviderInfo {
                name: "deepseek".into(),
                base_url: base_url.trim_end_matches('/').to_string(),
                model,
                capabilities: capabilities.clone(),
            },
            capabilities,
            max_retries: 2,
            retry_base_delay: Duration::from_millis(200),
        }
    }

    /// Override retry behavior. `max_retries` is the number of retries after the first attempt.
    pub fn with_retry(mut self, max_retries: usize, retry_base_delay: Duration) -> Self {
        self.max_retries = max_retries;
        self.retry_base_delay = retry_base_delay;
        self
    }

    fn build_request_body(&self, request: &ChatRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|msg| {
                let mut value = serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                });
                if let Some(tool_call_id) = &msg.tool_call_id {
                    value["tool_call_id"] = serde_json::Value::String(tool_call_id.clone());
                }
                if let Some(tool_calls) = &msg.tool_calls {
                    value["tool_calls"] = serde_json::Value::Array(tool_calls.clone());
                }
                value
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": request.stream,
        });

        if !request.tools.is_empty() {
            let tools: Vec<_> = request
                .tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.input_schema,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(tools);
        }
        if let Some(max) = request.max_tokens {
            body["max_tokens"] = max.into();
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = temp.into();
        }

        body
    }

    fn should_retry_status(status: StatusCode) -> bool {
        status == StatusCode::TOO_MANY_REQUESTS
            || status == StatusCode::REQUEST_TIMEOUT
            || status.is_server_error()
    }

    fn should_retry_error(error: &reqwest::Error) -> bool {
        error.is_timeout() || error.is_connect()
    }

    fn retry_delay(&self, attempt: usize) -> Duration {
        let multiplier = 1u32.checked_shl(attempt as u32).unwrap_or(u32::MAX);
        self.retry_base_delay
            .checked_mul(multiplier)
            .unwrap_or_else(|| Duration::from_secs(30))
    }

    async fn sleep_before_retry(&self, attempt: usize) {
        let delay = self.retry_delay(attempt);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
    }
}

fn build_http_client(base_url: &str) -> Client {
    let mut builder = Client::builder();
    if is_loopback_base_url(base_url) {
        builder = builder.no_proxy();
    }
    builder.build().unwrap_or_else(|_| Client::new())
}

fn is_loopback_base_url(base_url: &str) -> bool {
    reqwest::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(is_loopback_host))
        .unwrap_or(false)
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_matches(&['[', ']'][..]);
    host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|addr| addr.is_loopback())
}

#[async_trait]
impl ModelProvider for DeepSeekProvider {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn chat_stream(&self, request: ChatRequest) -> DeepCoderResult<Box<dyn StreamReceiver>> {
        let url = format!("{}/chat/completions", self.info.base_url);
        let body = self.build_request_body(&request);

        let mut attempt = 0usize;
        let response = loop {
            let result = self
                .client
                .post(&url)
                .bearer_auth(&self.api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match result {
                Ok(response) if response.status().is_success() => break response,
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    if attempt < self.max_retries && Self::should_retry_status(status) {
                        self.sleep_before_retry(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(DeepCoderError::Provider(format!(
                        "DeepSeek API returned {status}: {body}"
                    )));
                }
                Err(error) => {
                    if attempt < self.max_retries && Self::should_retry_error(&error) {
                        self.sleep_before_retry(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(DeepCoderError::Api(error));
                }
            }
        };

        Ok(Box::new(DeepSeekStream::new(
            response.bytes_stream().boxed(),
        )))
    }
}

/// DeepSeek SSE stream parser.
pub struct DeepSeekStream {
    stream: BoxStream<'static, Result<Bytes, reqwest::Error>>,
    buffer: String,
    done: bool,
    source_exhausted: bool,
    pending_tool_calls: BTreeMap<usize, PendingToolCall>,
    queued_events: VecDeque<DeepCoderResult<StreamEvent>>,
}

#[derive(Debug, Default)]
struct PendingToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl DeepSeekStream {
    fn new(stream: BoxStream<'static, Result<Bytes, reqwest::Error>>) -> Self {
        Self {
            stream,
            buffer: String::new(),
            done: false,
            source_exhausted: false,
            pending_tool_calls: BTreeMap::new(),
            queued_events: VecDeque::new(),
        }
    }

    #[cfg(test)]
    fn parse_line(line: &str) -> Option<DeepCoderResult<StreamEvent>> {
        let mut pending_tool_calls = BTreeMap::new();
        let mut queued_events = VecDeque::new();
        Self::parse_line_with_state(line, &mut pending_tool_calls, &mut queued_events)
    }

    fn parse_line_with_state(
        line: &str,
        pending_tool_calls: &mut BTreeMap<usize, PendingToolCall>,
        queued_events: &mut VecDeque<DeepCoderResult<StreamEvent>>,
    ) -> Option<DeepCoderResult<StreamEvent>> {
        let data = line.strip_prefix("data: ")?;
        if data == "[DONE]" {
            Self::queue_pending_tool_calls(pending_tool_calls, queued_events);
            queued_events.push_back(Ok(StreamEvent::Done));
            return queued_events.pop_front();
        }

        let json: serde_json::Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(e) => return Some(Err(DeepCoderError::Serialization(e))),
        };

        let delta = json
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))?;

        if let Some(reasoning) = delta.get("reasoning_content").and_then(|v| v.as_str())
            && !reasoning.is_empty()
        {
            return Some(Ok(StreamEvent::ReasoningDelta(reasoning.to_string())));
        }

        if let Some(content) = delta.get("content").and_then(|v| v.as_str())
            && !content.is_empty()
        {
            return Some(Ok(StreamEvent::TextDelta(content.to_string())));
        }

        if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
            Self::capture_tool_calls(tool_calls, pending_tool_calls, queued_events);
        }

        queued_events.pop_front()
    }

    fn capture_tool_calls(
        tool_calls: &[serde_json::Value],
        pending_tool_calls: &mut BTreeMap<usize, PendingToolCall>,
        queued_events: &mut VecDeque<DeepCoderResult<StreamEvent>>,
    ) {
        for (position, tool_call) in tool_calls.iter().enumerate() {
            let index = tool_call
                .get("index")
                .and_then(|value| value.as_u64())
                .unwrap_or(position as u64) as usize;
            let id = tool_call
                .get("id")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty());
            let function = tool_call.get("function");
            let name = function
                .and_then(|value| value.get("name"))
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty());
            let arguments = function
                .and_then(|value| value.get("arguments"))
                .and_then(|value| value.as_str());

            if !pending_tool_calls.contains_key(&index)
                && let (Some(name), Some(arguments)) = (name, arguments)
                && let Ok(arguments) = serde_json::from_str(arguments)
            {
                queued_events.push_back(Ok(StreamEvent::ToolCall {
                    id: id.unwrap_or_default().to_string(),
                    name: name.to_string(),
                    arguments,
                }));
                continue;
            }

            let pending = pending_tool_calls.entry(index).or_default();
            if let Some(id) = id {
                pending.id = Some(id.to_string());
            }
            if let Some(name) = name {
                pending.name = Some(name.to_string());
            }
            if let Some(arguments) = arguments {
                pending.arguments.push_str(arguments);
            }
        }
    }

    fn queue_pending_tool_calls(
        pending_tool_calls: &mut BTreeMap<usize, PendingToolCall>,
        queued_events: &mut VecDeque<DeepCoderResult<StreamEvent>>,
    ) {
        for (_, pending) in std::mem::take(pending_tool_calls) {
            let Some(name) = pending.name else {
                queued_events.push_back(Err(DeepCoderError::Provider(
                    "streamed tool call missing function name".into(),
                )));
                continue;
            };
            let arguments = if pending.arguments.trim().is_empty() {
                serde_json::json!({})
            } else {
                match serde_json::from_str(&pending.arguments) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        queued_events.push_back(Err(DeepCoderError::Provider(format!(
                            "invalid streamed tool call arguments for {name}: {error}"
                        ))));
                        continue;
                    }
                }
            };
            queued_events.push_back(Ok(StreamEvent::ToolCall {
                id: pending.id.unwrap_or_default(),
                name,
                arguments,
            }));
        }
    }
}

#[async_trait]
impl StreamReceiver for DeepSeekStream {
    async fn next_event(&mut self) -> Option<DeepCoderResult<StreamEvent>> {
        if let Some(event) = self.pop_queued_event() {
            return Some(event);
        }
        if self.done {
            return None;
        }
        if self.source_exhausted {
            self.done = true;
            return None;
        }

        loop {
            if let Some(pos) = self.buffer.find('\n') {
                let line = self.buffer[..pos].trim_end_matches('\r').to_string();
                self.buffer.drain(..=pos);
                if line.is_empty() {
                    continue;
                }
                if let Some(event) = Self::parse_line_with_state(
                    &line,
                    &mut self.pending_tool_calls,
                    &mut self.queued_events,
                ) {
                    return Some(self.mark_if_done(event));
                }
                if let Some(event) = self.pop_queued_event() {
                    return Some(event);
                }
                continue;
            }

            match self.stream.next().await {
                Some(Ok(bytes)) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&bytes));
                }
                Some(Err(e)) => return Some(Err(DeepCoderError::Api(e))),
                None => {
                    self.source_exhausted = true;
                    if self.buffer.trim().is_empty() {
                        Self::queue_pending_tool_calls(
                            &mut self.pending_tool_calls,
                            &mut self.queued_events,
                        );
                        return self.pop_queued_event();
                    }
                    let line = std::mem::take(&mut self.buffer);
                    if let Some(event) = Self::parse_line_with_state(
                        line.trim(),
                        &mut self.pending_tool_calls,
                        &mut self.queued_events,
                    ) {
                        return Some(self.mark_if_done(event));
                    }
                    Self::queue_pending_tool_calls(
                        &mut self.pending_tool_calls,
                        &mut self.queued_events,
                    );
                    return self.pop_queued_event();
                }
            }
        }
    }
}

impl DeepSeekStream {
    fn pop_queued_event(&mut self) -> Option<DeepCoderResult<StreamEvent>> {
        self.queued_events
            .pop_front()
            .map(|event| self.mark_if_done(event))
    }

    fn mark_if_done(
        &mut self,
        event: DeepCoderResult<StreamEvent>,
    ) -> DeepCoderResult<StreamEvent> {
        if matches!(&event, Ok(StreamEvent::Done)) {
            self.done = true;
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepcoder_error::DeepCoderError;
    use deepcoder_types::tool::ToolSpec;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    #[test]
    fn request_body_converts_tools_to_function_calling_shape() {
        let provider = DeepSeekProvider::new(
            "test-key".into(),
            "deepseek-chat".into(),
            "https://example.test".into(),
        );
        let request = ChatRequest {
            model: "deepseek-chat".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: serde_json::Value::String("hi".into()),
                tool_call_id: None,
                tool_calls: None,
            }],
            tools: vec![ToolSpec {
                name: "read_file".into(),
                description: "Read file".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"]
                }),
            }],
            stream: true,
            max_tokens: None,
            temperature: None,
        };

        let body = provider.build_request_body(&request);
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "read_file");
        assert_eq!(
            body["tools"][0]["function"]["parameters"]["properties"]["path"]["type"],
            "string"
        );
    }

    #[test]
    fn stream_parser_covers_text_reasoning_tool_done_and_bad_json() {
        let text_line = sse_delta_line(serde_json::json!({"content": "hello"}));
        match DeepSeekStream::parse_line(&text_line).unwrap().unwrap() {
            StreamEvent::TextDelta(text) => assert_eq!(text, "hello"),
            other => panic!("unexpected event: {other:?}"),
        }

        let reasoning_line = sse_delta_line(serde_json::json!({"reasoning_content": "thinking"}));
        match DeepSeekStream::parse_line(&reasoning_line)
            .unwrap()
            .unwrap()
        {
            StreamEvent::ReasoningDelta(text) => assert_eq!(text, "thinking"),
            other => panic!("unexpected event: {other:?}"),
        }

        let tool_line = sse_delta_line(serde_json::json!({
            "tool_calls": [{
                "id": "call_1",
                "function": {
                    "name": "read_file",
                    "arguments": "{\"path\":\"README.md\"}"
                }
            }]
        }));
        match DeepSeekStream::parse_line(&tool_line).unwrap().unwrap() {
            StreamEvent::ToolCall {
                id,
                name,
                arguments,
            } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
                assert_eq!(arguments["path"], "README.md");
            }
            other => panic!("unexpected event: {other:?}"),
        }

        assert!(matches!(
            DeepSeekStream::parse_line("data: [DONE]").unwrap().unwrap(),
            StreamEvent::Done
        ));
        assert!(matches!(
            DeepSeekStream::parse_line("data: {not-json}").unwrap(),
            Err(DeepCoderError::Serialization(_))
        ));
    }

    #[test]
    fn loopback_base_url_detection_covers_local_mock_hosts() {
        assert!(is_loopback_base_url("http://127.0.0.1:12345"));
        assert!(is_loopback_base_url("http://localhost:12345"));
        assert!(is_loopback_base_url("http://[::1]:12345"));
        assert!(!is_loopback_base_url("https://api.deepseek.com"));
    }

    #[tokio::test]
    async fn stream_parser_accumulates_chunked_tool_call_until_done() {
        let first_chunk = sse_delta_line(serde_json::json!({
            "tool_calls": [{
                "index": 0,
                "id": "call_1",
                "function": {
                    "name": "read_file",
                    "arguments": "{\"path\""
                }
            }]
        }));
        let second_chunk = sse_delta_line(serde_json::json!({
            "tool_calls": [{
                "index": 0,
                "function": {
                    "arguments": ":\"README.md\"}"
                }
            }]
        }));
        let mut stream = DeepSeekStream::new(
            futures::stream::iter(vec![
                Ok(Bytes::from(format!("{first_chunk}\n\n"))),
                Ok(Bytes::from(format!("{second_chunk}\n\n"))),
                Ok(Bytes::from_static(b"data: [DONE]\n\n")),
            ])
            .boxed(),
        );

        match stream.next_event().await.unwrap().unwrap() {
            StreamEvent::ToolCall {
                id,
                name,
                arguments,
            } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
                assert_eq!(arguments["path"], "README.md");
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(matches!(
            stream.next_event().await.unwrap().unwrap(),
            StreamEvent::Done
        ));
        assert!(stream.next_event().await.is_none());
    }

    #[tokio::test]
    async fn chat_stream_returns_classified_non_success_status() {
        let (base_url, handle) = spawn_http_server(vec![http_response(
            "400 Bad Request",
            "application/json",
            r#"{"error":"bad request"}"#,
        )])
        .await;
        let provider = DeepSeekProvider::new("test-key".into(), "deepseek-chat".into(), base_url)
            .with_retry(0, Duration::ZERO);

        let error = match provider.chat_stream(sample_request()).await {
            Ok(_) => panic!("expected non-success status to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, DeepCoderError::Provider(_)));
        assert!(error.to_string().contains("400 Bad Request"));

        let requests = handle.await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /chat/completions"));
    }

    #[tokio::test]
    async fn chat_stream_retries_retryable_status_then_streams() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
        let (base_url, handle) = spawn_http_server(vec![
            http_response("500 Internal Server Error", "text/plain", "retry"),
            http_response("200 OK", "text/event-stream", body),
        ])
        .await;
        let provider = DeepSeekProvider::new("test-key".into(), "deepseek-chat".into(), base_url)
            .with_retry(1, Duration::ZERO);

        let mut stream = provider.chat_stream(sample_request()).await.unwrap();
        match stream.next_event().await.unwrap().unwrap() {
            StreamEvent::TextDelta(text) => assert_eq!(text, "ok"),
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(matches!(
            stream.next_event().await.unwrap().unwrap(),
            StreamEvent::Done
        ));

        let requests = handle.await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            requests
                .iter()
                .all(|request| request.starts_with("POST /chat/completions"))
        );
    }

    #[tokio::test]
    async fn chat_stream_returns_api_error_when_connection_closes_before_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            drop(socket);
        });
        let provider = DeepSeekProvider::new(
            "test-key".into(),
            "deepseek-chat".into(),
            format!("http://{addr}"),
        )
        .with_retry(0, Duration::ZERO);

        let result = tokio::time::timeout(
            Duration::from_secs(5),
            provider.chat_stream(sample_request()),
        )
        .await
        .unwrap();
        let error = match result {
            Ok(_) => panic!("expected dropped connection to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, DeepCoderError::Api(_)));
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn stream_allows_eof_without_done_after_last_delta() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n";
        let (base_url, handle) =
            spawn_http_server(vec![http_response("200 OK", "text/event-stream", body)]).await;
        let provider = DeepSeekProvider::new("test-key".into(), "deepseek-chat".into(), base_url)
            .with_retry(0, Duration::ZERO);

        let mut stream = provider.chat_stream(sample_request()).await.unwrap();
        match stream.next_event().await.unwrap().unwrap() {
            StreamEvent::TextDelta(text) => assert_eq!(text, "partial"),
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(stream.next_event().await.is_none());

        let requests = handle.await.unwrap();
        assert_eq!(requests.len(), 1);
    }

    fn sse_delta_line(delta: serde_json::Value) -> String {
        format!(
            "data: {}",
            serde_json::json!({"choices": [{"delta": delta}]})
        )
    }

    fn sample_request() -> ChatRequest {
        ChatRequest {
            model: "deepseek-chat".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: serde_json::Value::String("hi".into()),
                tool_call_id: None,
                tool_calls: None,
            }],
            tools: Vec::new(),
            stream: true,
            max_tokens: None,
            temperature: None,
        }
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
                let mut buffer = vec![0u8; 8192];
                let size = socket.read(&mut buffer).await.unwrap();
                requests.push(String::from_utf8_lossy(&buffer[..size]).to_string());
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });

        (format!("http://{addr}"), handle)
    }
}
