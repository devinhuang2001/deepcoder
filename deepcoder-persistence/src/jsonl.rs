//! JSONL 事件日志

use std::path::Path;

/// 将事件写入 JSONL 文件
pub fn append_event(path: &Path, event: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(event)?)?;
    Ok(())
}
