//! Session 索引

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Session 索引条目
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionEntry {
    pub id: String,
    #[serde(default)]
    pub thread_id: String,
    pub model: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
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
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, content)
    }

    pub fn upsert(&mut self, entry: SessionEntry) {
        if let Some(existing) = self.sessions.iter_mut().find(|item| item.id == entry.id) {
            *existing = entry;
        } else {
            self.sessions.push(entry);
        }
        self.sort_by_updated_desc();
    }

    pub fn get(&self, id: &str) -> Option<&SessionEntry> {
        self.sessions.iter().find(|entry| entry.id == id)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|entry| entry.id != id);
        before != self.sessions.len()
    }

    pub fn sorted_entries(&self) -> Vec<SessionEntry> {
        let mut entries = self.sessions.clone();
        entries.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| right.created_at.cmp(&left.created_at))
        });
        entries
    }

    fn sort_by_updated_desc(&mut self) {
        self.sessions = self.sorted_entries();
    }
}
