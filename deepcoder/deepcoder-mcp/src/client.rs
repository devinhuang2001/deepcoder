//! MCP client integration.

use async_trait::async_trait;
use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_tools::{Tool, ToolContext, ToolRouter};
use deepcoder_types::tool::{JsonToolOutput, ToolExposure, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "inputSchema", alias = "input_schema", default)]
    pub input_schema: Value,
}

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn request(&self, method: &str, params: Value) -> DeepCoderResult<Value>;
}

pub struct McpClient {
    transport: Arc<dyn McpTransport>,
    capabilities: tokio::sync::RwLock<Option<Value>>,
}

impl McpClient {
    pub fn new(transport: Arc<dyn McpTransport>) -> Self {
        Self {
            transport,
            capabilities: tokio::sync::RwLock::new(None),
        }
    }

    pub async fn initialize(&self) -> DeepCoderResult<Value> {
        let result = self
            .transport
            .request(
                "initialize",
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "clientInfo": {
                        "name": "deepcoder",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {}
                }),
            )
            .await?;
        *self.capabilities.write().await = Some(result.clone());
        Ok(result)
    }

    pub async fn capabilities(&self) -> Option<Value> {
        self.capabilities.read().await.clone()
    }

    pub async fn list_tools(&self) -> DeepCoderResult<Vec<McpTool>> {
        let result = self
            .transport
            .request("tools/list", serde_json::json!({}))
            .await?;
        let tools_value = result
            .get("tools")
            .cloned()
            .ok_or_else(|| DeepCoderError::Mcp("tools/list response missing tools".into()))?;
        serde_json::from_value(tools_value).map_err(DeepCoderError::Serialization)
    }

    pub async fn register_tools(&self, router: &ToolRouter) -> DeepCoderResult<Vec<ToolSpec>> {
        let tools = self.list_tools().await?;
        let specs = tools.iter().map(mcp_tool_to_spec).collect::<Vec<_>>();
        let client = Arc::new(self.clone_for_tools());
        for tool in tools {
            router
                .register(Arc::new(McpToolWrapper::new(tool, client.clone())))
                .await;
        }
        Ok(specs)
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> DeepCoderResult<JsonToolOutput> {
        let result = self
            .transport
            .request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": arguments
                }),
            )
            .await?;
        Ok(mcp_result_to_tool_output(result))
    }

    fn clone_for_tools(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            capabilities: tokio::sync::RwLock::new(None),
        }
    }
}

pub fn mcp_tool_to_spec(tool: &McpTool) -> ToolSpec {
    ToolSpec {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: if tool.input_schema.is_null() {
            serde_json::json!({"type": "object", "properties": {}})
        } else {
            tool.input_schema.clone()
        },
    }
}

struct McpToolWrapper {
    name: &'static str,
    spec: ToolSpec,
    client: Arc<McpClient>,
}

impl McpToolWrapper {
    fn new(tool: McpTool, client: Arc<McpClient>) -> Self {
        let name: &'static str = Box::leak(tool.name.clone().into_boxed_str());
        Self {
            name,
            spec: mcp_tool_to_spec(&tool),
            client,
        }
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    async fn call(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        self.client.call_tool(self.name, params).await
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}

fn mcp_result_to_tool_output(result: Value) -> JsonToolOutput {
    let is_error = result
        .get("isError")
        .or_else(|| result.get("is_error"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let content = result
        .get("content")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(|value| value.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
        .map(Value::String)
        .unwrap_or(result);

    JsonToolOutput {
        data: content,
        is_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepcoder_types::tool::ToolCall;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockTransport {
        requests: Mutex<Vec<(String, Value)>>,
        responses: Mutex<VecDeque<Value>>,
    }

    impl MockTransport {
        fn new(responses: Vec<Value>) -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(VecDeque::from(responses)),
            }
        }
    }

    #[async_trait]
    impl McpTransport for MockTransport {
        async fn request(&self, method: &str, params: Value) -> DeepCoderResult<Value> {
            self.requests
                .lock()
                .unwrap()
                .push((method.to_string(), params));
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| DeepCoderError::Mcp("missing mock response".into()))
        }
    }

    #[test]
    fn mcp_tool_spec_conversion() {
        let tool = McpTool {
            name: "external.echo".into(),
            description: "Echo text".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}}
            }),
        };
        let spec = mcp_tool_to_spec(&tool);
        assert_eq!(spec.name, "external.echo");
        assert_eq!(spec.description, "Echo text");
        assert_eq!(spec.input_schema["properties"]["text"]["type"], "string");
    }

    #[tokio::test]
    async fn mcp_client_initialize_and_list_tools() {
        let transport = Arc::new(MockTransport::new(vec![
            serde_json::json!({"capabilities": {"tools": {}}}),
            serde_json::json!({
                "tools": [{
                    "name": "external.echo",
                    "description": "Echo text",
                    "inputSchema": {"type": "object"}
                }]
            }),
        ]));
        let client = McpClient::new(transport.clone());

        let capabilities = client.initialize().await.unwrap();
        assert_eq!(capabilities["capabilities"]["tools"], serde_json::json!({}));
        assert!(client.capabilities().await.is_some());

        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools[0].name, "external.echo");

        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests[0].0, "initialize");
        assert_eq!(requests[1].0, "tools/list");
    }

    #[tokio::test]
    async fn mcp_client_registers_and_calls_tool() {
        let transport = Arc::new(MockTransport::new(vec![
            serde_json::json!({
                "tools": [{
                    "name": "external.echo",
                    "description": "Echo text",
                    "inputSchema": {"type": "object"}
                }]
            }),
            serde_json::json!({
                "content": [{"type": "text", "text": "hello"}],
                "isError": false
            }),
        ]));
        let client = McpClient::new(transport.clone());
        let router = ToolRouter::new();
        client.register_tools(&router).await.unwrap();

        let output = router
            .execute(
                &ToolCall {
                    call_id: "call_echo".into(),
                    tool_name: "external.echo".into(),
                    arguments: serde_json::json!({"text": "hello"}),
                },
                &ToolContext {
                    config: deepcoder_config::Config::load_default().unwrap(),
                    workspace_root: std::env::current_dir().ok(),
                    tool_router: None,
                    tool_approver: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(output.data, "hello");
        assert!(!output.is_error);

        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests[0].0, "tools/list");
        assert_eq!(requests[1].0, "tools/call");
        assert_eq!(requests[1].1["name"], "external.echo");
    }
}
