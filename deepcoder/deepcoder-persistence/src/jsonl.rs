//! JSONL 事件日志

use std::path::Path;

/// 将事件写入 JSONL 文件
pub fn append_event(path: &Path, event: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let line = serde_json::to_string(event).map_err(std::io::Error::other)?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// 读取 JSONL 文件中的事件；空文件或不存在返回空列表。
pub fn read_events(path: &Path) -> std::io::Result<Vec<serde_json::Value>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(std::io::Error::other))
        .collect()
}
