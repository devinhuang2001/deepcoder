use deepcoder_error::{DeepCoderError, DeepCoderResult};
use std::path::{Component, Path, PathBuf};

use crate::ToolContext;

pub(crate) fn workspace_root(ctx: &ToolContext) -> DeepCoderResult<PathBuf> {
    let root = match &ctx.workspace_root {
        Some(root) => root.clone(),
        None => std::env::current_dir()?,
    };
    Ok(root.canonicalize()?)
}

pub(crate) fn resolve_workspace_path(
    raw_path: &str,
    ctx: &ToolContext,
) -> DeepCoderResult<PathBuf> {
    let root = workspace_root(ctx)?;
    let input = PathBuf::from(raw_path);
    let candidate = if input.is_absolute() {
        input
    } else {
        root.join(input)
    };
    let normalized = lexical_normalize(&candidate);
    let Some(existing_ancestor) = nearest_existing_ancestor(&normalized) else {
        return Err(DeepCoderError::ToolExecution(format!(
            "path has no existing ancestor: {raw_path}"
        )));
    };
    let canonical_ancestor = existing_ancestor.canonicalize()?;
    if !canonical_ancestor.starts_with(&root) {
        return Err(DeepCoderError::ToolDenied {
            reason: format!("path outside workspace: {raw_path}"),
        });
    }
    let tail = normalized
        .strip_prefix(&existing_ancestor)
        .unwrap_or_else(|_| Path::new(""));
    Ok(canonical_ancestor.join(tail))
}

pub(crate) fn ensure_path_in_workspace(path: &Path, ctx: &ToolContext) -> DeepCoderResult<()> {
    let root = workspace_root(ctx)?;
    let normalized = lexical_normalize(path);
    let Some(existing_ancestor) = nearest_existing_ancestor(&normalized) else {
        return Err(DeepCoderError::ToolExecution(format!(
            "path has no existing ancestor: {}",
            path.display()
        )));
    };
    let canonical_ancestor = existing_ancestor.canonicalize()?;
    if canonical_ancestor.starts_with(root) {
        Ok(())
    } else {
        Err(DeepCoderError::ToolDenied {
            reason: format!("path outside workspace: {}", path.display()),
        })
    }
}

fn nearest_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut current = path;
    loop {
        if current.exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}
