//! Search tools.

use async_trait::async_trait;
use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_types::tool::{JsonToolOutput, ToolSpec};
use regex::Regex;
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::{Tool, ToolContext};

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &'static str {
        "glob"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Return file paths matching a glob pattern.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "max_results": {"type": "integer", "minimum": 1}
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
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
        let pattern = required_str(&params, "pattern")?;
        let glob_pattern = workspace_glob_pattern(pattern, ctx)?;
        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(500) as usize;
        let mut matches = Vec::new();
        for entry in glob::glob(&glob_pattern)
            .map_err(|e| DeepCoderError::ToolExecution(format!("invalid glob: {e}")))?
        {
            let path = entry.map_err(|e| DeepCoderError::ToolExecution(e.to_string()))?;
            if crate::path::ensure_path_in_workspace(&path, ctx).is_err() {
                continue;
            }
            matches.push(path.display().to_string());
            if matches.len() >= max_results {
                break;
            }
        }
        Ok(JsonToolOutput::success(json!({
            "pattern": pattern,
            "matches": matches
        })))
    }
}

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Search text files recursively with a regular expression.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"},
                    "max_results": {"type": "integer", "minimum": 1}
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
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
        let pattern = required_str(&params, "pattern")?;
        let regex = Regex::new(pattern)
            .map_err(|e| DeepCoderError::ToolExecution(format!("invalid regex: {e}")))?;
        let root = params
            .get("path")
            .and_then(|v| v.as_str())
            .map(|path| crate::path::resolve_workspace_path(path, ctx))
            .transpose()?
            .unwrap_or(crate::path::workspace_root(ctx)?);
        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(500) as usize;

        let mut results = Vec::new();
        visit_files(&root, ctx, &mut |path| {
            if results.len() >= max_results {
                return Ok(());
            }
            if let Ok(content) = std::fs::read_to_string(path) {
                for (idx, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        results.push(json!({
                            "path": path.display().to_string(),
                            "line_number": idx + 1,
                            "line": line
                        }));
                        if results.len() >= max_results {
                            break;
                        }
                    }
                }
            }
            Ok(())
        })?;

        Ok(JsonToolOutput::success(json!({
            "pattern": pattern,
            "root": root.display().to_string(),
            "matches": results
        })))
    }
}

fn required_str<'a>(params: &'a serde_json::Value, name: &str) -> DeepCoderResult<&'a str> {
    params
        .get(name)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| DeepCoderError::ToolExecution(format!("missing required string: {name}")))
}

fn workspace_glob_pattern(pattern: &str, ctx: &ToolContext) -> DeepCoderResult<String> {
    let root = crate::path::workspace_root(ctx)?;
    let pattern_path = PathBuf::from(pattern);
    let candidate = if pattern_path.is_absolute() {
        pattern_path
    } else {
        root.join(pattern_path)
    };
    let static_prefix = glob_static_prefix(&candidate);
    crate::path::ensure_path_in_workspace(&static_prefix, ctx)?;
    Ok(candidate.display().to_string())
}

fn glob_static_prefix(path: &Path) -> PathBuf {
    let mut prefix = PathBuf::new();
    for component in path.components() {
        let text = component.as_os_str().to_string_lossy();
        if text.contains('*') || text.contains('?') || text.contains('[') {
            break;
        }
        prefix.push(component.as_os_str());
    }
    if prefix.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        prefix
    }
}

fn visit_files<F>(path: &Path, ctx: &ToolContext, visitor: &mut F) -> DeepCoderResult<()>
where
    F: FnMut(&Path) -> DeepCoderResult<()>,
{
    if crate::path::ensure_path_in_workspace(path, ctx).is_err() {
        return Ok(());
    }
    if path.is_file() {
        visitor(path)?;
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if matches!(name, ".git" | "target" | "node_modules") {
                continue;
            }
            visit_files(&path, ctx, visitor)?;
        } else if path.is_file() {
            visitor(&path)?;
        }
    }
    Ok(())
}
