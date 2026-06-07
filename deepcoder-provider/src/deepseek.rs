//! DeepSeek V4 Provider 实现

use async_trait::async_trait;
use reqwest::Client;
use deepcoder_types::provider::*;
use deepcoder_error::DeepCoderResult;

use super::{ModelProvider, StreamReceiver};

/// DeepSeek V4 Provider
pub struct DeepSeekProvider {
    client: Client,
    api_key: String,
    info: ProviderInfo,
    capabilities: ProviderCapabilities,
}

impl DeepSeekProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            capabilities: ProviderCapabilities::default(),
            info: ProviderInfo {
                name: "deepseek".into(),
                base_url,
                model,
                capabilities: ProviderCapabilities::default(),
            },
        }
    }

    fn build_request_body(&self, request: &ChatRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request.messages.iter().map(|msg| {
            serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            })
        }).collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": request.stream,
        });

        if let Some(tools) = request.tools.as_ref().filter(|t| !t.is_empty()) {
            body["tools"] = serde_json::to_value(tools).unwrap_or_default();
        }
        if let Some(max) = request.max_tokens { body["max_tokens"] = max.into(); }
        if let Some(temp) = request.temperature { body["temperature"] = temp.into(); }

        body
    }
}

#[async_trait]
impl ModelProvider for DeepSeekProvider {
    fn info(&self) -> &ProviderInfo { &self.info }
    fn capabilities(&self) -> &ProviderCapabilities { &self.capabilities }

    async fn chat_stream(&self, request: ChatRequest) -> DeepCoderResult<Box<dyn StreamReceiver>> {
        let url = format!("{}/chat/completions", self.info.base_url);
        let body = self.build_request_body(&request);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        Ok(Box::new(DeepSeekStream::new(response)))
    }
}

/// DeepSeek SSE 流解析
pub struct DeepSeekStream {
    response: reqwest::Response,
    buffer: String,
}

impl DeepSeekStream {
    pub fn new(response: reqwest::Response) -> Self {
        Self { response, buffer: String::new() }
    }
}

impl DeepSeekStream {
    fn parse_line(&mut self, line: &str) -> Option<DeepCoderResult<StreamEvent>> {
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                return Some(Ok(StreamEvent::Done));
            }
            // 解析 JSON
            match serde_json::from_str::<serde_json::Value>(data) {
                Ok(json) => {
                    let choices = json.get("choices")?.as_array()?;
                    let delta = choices.first()?.get("delta")?;

                    // 检查 reasoning_content (DeepSeek V4 特有)
                    if let Some(reasoning) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                        if !reasoning.is_empty() {
                            return Some(Ok(StreamEvent::ReasoningDelta(reasoning.to_string())));
                        }
                    }

                    // 检查 content
                    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                        if !content.is_empty() {
                            return Some(Ok(StreamEvent::TextDelta(content.to_string())));
                        }
                    }

                    // 检查 tool_calls
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                        for tc in tool_calls {
                            if let (Some(id), Some(name), Some(args)) = (
                                tc.get("id").and_then(|v| v.as_str()),
                                tc.get("function").and_then(|f| f.get("name").and_then(|v| v.as_str())),
                                tc.get("function").and_then(|f| f.get("arguments")),
                            ) {
                                return Some(Ok(StreamEvent::ToolCall {
                                    id: id.to_string(),
                                    name: name.to_string(),
                                    arguments: serde_json::from_str(args.as_str().unwrap_or("{}")).unwrap_or_default(),
                                }));
                            }
                        }
                    }

                    None
                }
                Err(e) => Some(Err(DeepCoderError::from(format!("SSE 解析错误: {e}")))),
            }
        } else {
            None
        }
    }
}

#[async_trait]
impl StreamReceiver for DeepSeekStream {
    async fn next_event(&mut self) -> Option<DeepCoderResult<StreamEvent>> {
        // 简化实现：从 buffer 中读取行
        // 生产版本应使用完整 SSE 解析器
        None // placeholder
    }
}
