//! MCP server integration.

use anyhow::Result;
use deepcoder_tools::{ToolContext, ToolRouter};
use deepcoder_types::tool::ToolCall;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};

#[derive(Debug, Deserialize)]
struct McpRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

pub struct McpServer {
    config: deepcoder_config::Config,
    router: Arc<ToolRouter>,
}

impl McpServer {
    pub fn new(config: deepcoder_config::Config) -> Self {
        Self {
            config,
            router: Arc::new(ToolRouter::with_builtins()),
        }
    }

    pub async fn process_json(&self, input: &str) -> Value {
        match serde_json::from_str::<McpRequest>(input) {
            Ok(request) => self.process(request).await,
            Err(error) => error_response(None, -32700, format!("Parse error: {error}")),
        }
    }

    async fn process(&self, request: McpRequest) -> Value {
        let id = request.id.clone();
        if request.jsonrpc != "2.0" {
            return error_response(id, -32600, "Invalid Request: jsonrpc must be 2.0");
        }

        match request.method.as_str() {
            "initialize" => success_response(
                id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "deepcoder-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "tools": {}
                    }
                }),
            ),
            "tools/list" => self.tools_list(id).await,
            "tools/call" => self.tools_call(id, request.params).await,
            _ => error_response(id, -32601, format!("Method not found: {}", request.method)),
        }
    }

    async fn tools_list(&self, id: Option<Value>) -> Value {
        let mut tools: Vec<_> = self
            .router
            .direct_specs()
            .await
            .into_iter()
            .map(|spec| {
                serde_json::json!({
                    "name": spec.name,
                    "description": spec.description,
                    "inputSchema": spec.input_schema
                })
            })
            .collect();
        tools.sort_by(|left, right| {
            left["name"]
                .as_str()
                .unwrap_or_default()
                .cmp(right["name"].as_str().unwrap_or_default())
        });
        success_response(id, serde_json::json!({ "tools": tools }))
    }

    async fn tools_call(&self, id: Option<Value>, params: Option<Value>) -> Value {
        let Some(params) = params.as_ref() else {
            return error_response(id, -32602, "Invalid params: missing params");
        };
        let Some(name) = params
            .get("name")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        else {
            return error_response(id, -32602, "Invalid params: missing name");
        };
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let ctx = ToolContext {
            config: self.config.clone(),
            workspace_root: std::env::current_dir().ok(),
            tool_router: Some(self.router.clone()),
            tool_approver: None,
        };
        let call = ToolCall {
            call_id: format!("mcp_{name}"),
            tool_name: name.to_string(),
            arguments,
        };

        match self.router.execute(&call, &ctx).await {
            Ok(output) => success_response(id, tool_result(output.data, output.is_error)),
            Err(error) => success_response(id, tool_result(Value::String(error.to_string()), true)),
        }
    }
}

pub async fn run_stdio(config: deepcoder_config::Config) -> Result<()> {
    let server = McpServer::new(config);
    let mut lines = io::BufReader::new(io::stdin()).lines();
    let mut stdout = io::stdout();
    while let Some(line) = lines.next_line().await? {
        tracing::debug!("MCP stdin: {line}");
        let response = server.process_json(&line).await;
        stdout.write_all(response.to_string().as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
    Ok(())
}

fn success_response(id: Option<Value>, result: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn error_response(id: Option<Value>, code: i32, message: impl Into<String>) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}

fn tool_result(data: Value, is_error: bool) -> Value {
    let text = match data {
        Value::String(text) => text,
        other => serde_json::to_string_pretty(&other).unwrap_or_else(|_| other.to_string()),
    };
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "isError": is_error
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn server() -> McpServer {
        McpServer::new(deepcoder_config::Config::load_default().unwrap())
    }

    fn temp_file() -> PathBuf {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("deepcoder_mcp_test_{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).ok();
        }
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("note.txt");
        std::fs::write(&file, "hello mcp").unwrap();
        file
    }

    #[tokio::test]
    async fn initialize_returns_server_info() {
        let response = server()
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
            .await;
        assert_eq!(response["result"]["serverInfo"]["name"], "deepcoder-mcp");
    }

    #[tokio::test]
    async fn tools_list_includes_builtin_tools() {
        let response = server()
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
            .await;
        let tools = response["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|tool| tool["name"] == "read_file"));
    }

    #[tokio::test]
    async fn tools_call_executes_builtin_tool() {
        let file = temp_file();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "read_file",
                "arguments": {
                    "path": file.display().to_string()
                }
            }
        });
        let response = server().process_json(&request.to_string()).await;
        assert_eq!(response["result"]["isError"], false);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("hello mcp")
        );
    }

    #[tokio::test]
    async fn tools_call_missing_name_returns_invalid_params() {
        let response = server()
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}"#)
            .await;
        assert_eq!(response["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let response = server()
            .process_json(r#"{"jsonrpc":"2.0","id":1,"method":"missing"}"#)
            .await;
        assert_eq!(response["error"]["code"], -32601);
    }
}
