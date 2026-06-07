//! Session 索引

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Session 索引条目
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    pub model: String,
    pub created_at: String,
    pub message_count: u32,
    pub summary: Option<String>,
}

/// Session 索引
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SessionIndex {
    pub sessions: Vec<SessionEntry>,
}

impl SessionIndex {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let content = toml::to_string_pretty(self).unwrap();
        std::fs::write(path, content)
    }
}
