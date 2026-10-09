//! File tools.

use async_trait::async_trait;
use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_types::tool::{JsonToolOutput, ToolSpec};
use serde_json::json;
use std::path::PathBuf;

use crate::{Tool, ToolContext};

const MAX_READ_BYTES: u64 = 512 * 1024;

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Read a UTF-8 text file and return content with line numbers.".into(),
            input_schema: schema(
                json!({
                    "path": {"type": "string"},
                    "max_bytes": {"type": "integer", "minimum": 1}
                }),
                &["path"],
            ),
        }
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let path = path_param(&params, ctx)?;
        let max_bytes = params
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(MAX_READ_BYTES);
        let metadata = tokio::fs::metadata(&path).await?;
        if !metadata.is_file() {
            return Err(DeepCoderError::ToolExecution(format!(
                "not a file: {}",
                path.display()
            )));
        }
        if metadata.len() > max_bytes {
            return Err(DeepCoderError::ToolExecution(format!(
                "file too large: {} bytes exceeds limit {}",
                metadata.len(),
                max_bytes
            )));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let lines: Vec<_> = content
            .lines()
            .enumerate()
            .map(|(idx, text)| json!({"number": idx + 1, "text": text}))
            .collect();

        Ok(JsonToolOutput::success(json!({
            "path": path.display().to_string(),
            "content": content,
            "lines": lines
        })))
    }
}

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Create or overwrite a UTF-8 text file.".into(),
            input_schema: schema(
                json!({
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                }),
                &["path", "content"],
            ),
        }
    }

    async fn call(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let path = path_param(&params, ctx)?;
        let content = required_string(&params, "content")?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, content.as_bytes()).await?;
        Ok(JsonToolOutput::success(json!({
            "path": path.display().to_string(),
            "bytes": content.len()
        })))
    }
}

pub struct FileEditTool;

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Replace one exact string in a UTF-8 text file and return a simple diff."
                .into(),
            input_schema: schema(
                json!({
                    "path": {"type": "string"},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"}
                }),
                &["path", "old_string", "new_string"],
            ),
        }
    }

    async fn call(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let path = path_param(&params, ctx)?;
        let old_string = required_string(&params, "old_string")?;
        let new_string = params
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DeepCoderError::ToolExecution("missing required string: new_string".into())
            })?
            .to_string();

        let content = tokio::fs::read_to_string(&path).await?;
        let matches = content.matches(&old_string).count();
        if matches == 0 {
            return Err(DeepCoderError::ToolExecution(format!(
                "old_string not found in {}",
                path.display()
            )));
        }
        if matches > 1 {
            return Err(DeepCoderError::ToolExecution(format!(
                "old_string matched {matches} times in {}; refusing ambiguous edit",
                path.display()
            )));
        }

        let updated = content.replacen(&old_string, &new_string, 1);
        tokio::fs::write(&path, updated.as_bytes()).await?;
        Ok(JsonToolOutput::success(json!({
            "path": path.display().to_string(),
            "replacements": 1,
            "diff": simple_diff(&old_string, &new_string)
        })))
    }
}

fn required_string(params: &serde_json::Value, name: &str) -> DeepCoderResult<String> {
    params
        .get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| DeepCoderError::ToolExecution(format!("missing required string: {name}")))
}

fn path_param(params: &serde_json::Value, ctx: &ToolContext) -> DeepCoderResult<PathBuf> {
    crate::path::resolve_workspace_path(&required_string(params, "path")?, ctx)
}

fn schema(properties: serde_json::Value, required: &[&str]) -> serde_json::Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn simple_diff(old_string: &str, new_string: &str) -> String {
    old_string
        .lines()
        .map(|line| format!("-{line}"))
        .chain(new_string.lines().map(|line| format!("+{line}")))
        .collect::<Vec<_>>()
        .join("\n")
}
