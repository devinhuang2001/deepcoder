//! Shell tools.

use async_trait::async_trait;
use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_sandbox::policy::{ExecPolicy, PolicyDecision};
use deepcoder_types::tool::{JsonToolOutput, PermissionResult, ToolSpec};
use serde_json::json;
use std::time::Instant;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

use crate::{Tool, ToolContext};

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Run a shell command with timeout and sandbox policy checks.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout_ms": {"type": "integer", "minimum": 1},
                    "workdir": {"type": "string"}
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        }
    }

    fn check_permissions(&self, input: &serde_json::Value) -> PermissionResult {
        let Some(command) = input.get("command").and_then(|v| v.as_str()) else {
            return PermissionResult::Deny {
                reason: "missing required string: command".into(),
            };
        };

        match ExecPolicy::new().evaluate(command) {
            PolicyDecision::Allow => PermissionResult::Allow,
            PolicyDecision::Deny(reason) => PermissionResult::Deny { reason },
            PolicyDecision::Prompt => PermissionResult::Prompt {
                message: format!("command requires approval: {command}"),
            },
        }
    }

    async fn call(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                DeepCoderError::ToolExecution("missing required string: command".into())
            })?;
        let timeout_ms = params
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(30_000);
        let sandbox = deepcoder_sandbox::SandboxManager::new(&ctx.config.sandbox.mode);
        if let Err(reason) = sandbox.check_command(command) {
            return Err(DeepCoderError::ToolDenied { reason });
        }

        let mut child = shell_command(command);
        if let Some(workdir) = params.get("workdir").and_then(|v| v.as_str()) {
            child.current_dir(workdir);
        }

        let started = Instant::now();
        let output = match timeout(Duration::from_millis(timeout_ms), child.output()).await {
            Ok(output) => output?,
            Err(_) => {
                return Err(DeepCoderError::ToolExecution(format!(
                    "command timed out after {timeout_ms}ms"
                )));
            }
        };
        let duration_ms = started.elapsed().as_millis() as u64;

        Ok(JsonToolOutput::success(json!({
            "command": command,
            "exit_code": output.status.code(),
            "success": output.status.success(),
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "duration_ms": duration_ms
        })))
    }
}

#[cfg(target_os = "windows")]
fn shell_command(command: &str) -> Command {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoLogo", "-NoProfile", "-Command", command]);
    cmd
}

#[cfg(not(target_os = "windows"))]
fn shell_command(command: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command]);
    cmd
}
