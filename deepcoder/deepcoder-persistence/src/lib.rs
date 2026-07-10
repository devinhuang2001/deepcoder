//! DeepCoder 持久化系统
//!
//! JSONL 事件日志 + TOML session 索引

pub mod index;
pub mod jsonl;

use deepcoder_types::message::Message;
use deepcoder_types::session::Thread;
use index::{SessionEntry, SessionIndex};
use std::path::PathBuf;

/// 持久化管理器
#[derive(Debug, Clone)]
pub struct Persistence {
    data_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub session_id: uuid::Uuid,
    pub thread: Thread,
    pub messages: Vec<Message>,
}

impl Persistence {
    pub fn new(data_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&data_dir).ok();
        std::fs::create_dir_all(data_dir.join("sessions")).ok();
        Self { data_dir }
    }

    /// 获取 JSONL 日志文件路径（按日分片）
    pub fn session_log_path(&self, session_id: &uuid::Uuid) -> PathBuf {
        let date = chrono::Utc::now().format("%Y-%m-%d");
        self.data_dir
            .join("sessions")
            .join(format!("{session_id}_{date}.jsonl"))
    }

    /// 获取所有 JSONL 日志文件路径（按文件名排序）。
    pub fn session_log_paths(&self, session_id: &uuid::Uuid) -> std::io::Result<Vec<PathBuf>> {
        let sessions_dir = self.data_dir.join("sessions");
        let prefix = format!("{session_id}_");
        let mut paths = Vec::new();
        if !sessions_dir.exists() {
            return Ok(paths);
        }
        for entry in std::fs::read_dir(sessions_dir)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if name.starts_with(&prefix) && name.ends_with(".jsonl") {
                paths.push(path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    /// 获取 session 索引路径
    pub fn index_path(&self) -> PathBuf {
        self.data_dir.join("sessions.toml")
    }

    pub fn append_session_event(
        &self,
        session_id: &uuid::Uuid,
        event_type: &str,
        payload: serde_json::Value,
    ) -> std::io::Result<()> {
        let event = serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "type": event_type,
            "payload": payload
        });
        jsonl::append_event(&self.session_log_path(session_id), &event)
    }

    pub fn record_message(
        &self,
        session_id: &uuid::Uuid,
        thread: &Thread,
        message: &Message,
    ) -> std::io::Result<()> {
        self.upsert_thread(session_id, thread, None)?;
        self.append_session_event(
            session_id,
            "message",
            serde_json::json!({
                "thread_id": thread.id,
                "message": message
            }),
        )
    }

    pub fn upsert_thread(
        &self,
        session_id: &uuid::Uuid,
        thread: &Thread,
        summary: Option<String>,
    ) -> std::io::Result<()> {
        let mut index = SessionIndex::load(&self.index_path());
        index.upsert(SessionEntry {
            id: session_id.to_string(),
            thread_id: thread.id.to_string(),
            model: thread.model.clone(),
            created_at: thread.created_at.to_rfc3339(),
            updated_at: thread.updated_at.to_rfc3339(),
            message_count: thread.message_count,
            summary,
        });
        index.save(&self.index_path())
    }

    pub fn list_sessions(&self) -> Vec<SessionEntry> {
        SessionIndex::load(&self.index_path()).sorted_entries()
    }

    pub fn delete_session(&self, session_id: &uuid::Uuid) -> std::io::Result<bool> {
        let mut index = SessionIndex::load(&self.index_path());
        let removed = index.remove(&session_id.to_string());
        index.save(&self.index_path())?;
        for path in self.session_log_paths(session_id)? {
            std::fs::remove_file(path).ok();
        }
        Ok(removed)
    }

    pub fn load_snapshot(
        &self,
        session_id: &uuid::Uuid,
    ) -> std::io::Result<Option<SessionSnapshot>> {
        let index = SessionIndex::load(&self.index_path());
        let Some(entry) = index.get(&session_id.to_string()).cloned() else {
            return Ok(None);
        };

        let thread_id = entry
            .thread_id
            .parse()
            .unwrap_or_else(|_| uuid::Uuid::new_v4());
        let created_at = parse_datetime(&entry.created_at);
        let updated_at = parse_datetime(if entry.updated_at.is_empty() {
            &entry.created_at
        } else {
            &entry.updated_at
        });
        let mut messages = Vec::new();
        for path in self.session_log_paths(session_id)? {
            for event in jsonl::read_events(&path)? {
                if event.get("type").and_then(|value| value.as_str()) != Some("message") {
                    continue;
                }
                let Some(message_value) = event
                    .get("payload")
                    .and_then(|payload| payload.get("message"))
                else {
                    continue;
                };
                let message = serde_json::from_value::<Message>(message_value.clone())
                    .map_err(std::io::Error::other)?;
                messages.push(message);
            }
        }

        Ok(Some(SessionSnapshot {
            session_id: *session_id,
            thread: Thread {
                id: thread_id,
                model: entry.model,
                created_at,
                updated_at,
                message_count: entry.message_count,
                token_usage: Default::default(),
                metadata: Default::default(),
            },
            messages,
        }))
    }

    pub fn fork_session(
        &self,
        session_id: &uuid::Uuid,
    ) -> std::io::Result<Option<SessionSnapshot>> {
        let Some(snapshot) = self.load_snapshot(session_id)? else {
            return Ok(None);
        };
        let fork_session_id = uuid::Uuid::new_v4();
        let mut fork_thread = snapshot.thread.clone();
        fork_thread.id = uuid::Uuid::new_v4();
        fork_thread.created_at = chrono::Utc::now();
        fork_thread.updated_at = fork_thread.created_at;
        fork_thread.message_count = snapshot.messages.len() as u32;
        fork_thread
            .metadata
            .insert("forked_from".into(), session_id.to_string());

        self.upsert_thread(
            &fork_session_id,
            &fork_thread,
            Some(format!("Fork of {session_id}")),
        )?;
        self.append_session_event(
            &fork_session_id,
            "session_forked",
            serde_json::json!({
                "from_session_id": session_id,
                "thread_id": fork_thread.id
            }),
        )?;
        for message in &snapshot.messages {
            self.record_message(&fork_session_id, &fork_thread, message)?;
        }

        Ok(Some(SessionSnapshot {
            session_id: fork_session_id,
            thread: fork_thread,
            messages: snapshot.messages,
        }))
    }
}

fn parse_datetime(value: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepcoder_types::message::{Message, MessageRole};
    use deepcoder_types::session::TokenUsage;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "deepcoder_persistence_{name}_{}",
            std::process::id()
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn thread(model: &str) -> Thread {
        Thread {
            id: uuid::Uuid::new_v4(),
            model: model.into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            message_count: 0,
            token_usage: TokenUsage::default(),
            metadata: Default::default(),
        }
    }

    #[test]
    fn jsonl_append_event_writes_one_line() {
        let path = temp_dir("jsonl").join("events.jsonl");
        jsonl::append_event(&path, &serde_json::json!({"type": "turn_start"})).unwrap();
        let events = jsonl::read_events(&path).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "turn_start");
    }

    #[test]
    fn session_index_load_missing_returns_default() {
        let path = temp_dir("missing_index").join("sessions.toml");
        let index = SessionIndex::load(&path);
        assert!(index.sessions.is_empty());
    }

    #[test]
    fn session_index_save_load_and_sort() {
        let path = temp_dir("index").join("sessions.toml");
        let mut index = SessionIndex::default();
        index.upsert(SessionEntry {
            id: "old".into(),
            thread_id: "thread-old".into(),
            model: "model-a".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            message_count: 1,
            summary: None,
        });
        index.upsert(SessionEntry {
            id: "new".into(),
            thread_id: "thread-new".into(),
            model: "model-b".into(),
            created_at: "2026-01-02T00:00:00Z".into(),
            updated_at: "2026-01-02T00:00:00Z".into(),
            message_count: 2,
            summary: Some("summary".into()),
        });
        index.save(&path).unwrap();

        let loaded = SessionIndex::load(&path);
        assert_eq!(loaded.sessions[0].id, "new");
        assert_eq!(loaded.sessions[1].id, "old");
    }

    #[test]
    fn session_log_path_contains_date() {
        let persistence = Persistence::new(temp_dir("paths"));
        let session_id = uuid::Uuid::new_v4();
        let path = persistence.session_log_path(&session_id);
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with(&session_id.to_string()));
        assert!(name.ends_with(".jsonl"));
        assert!(path.parent().unwrap().ends_with("sessions"));
    }

    #[test]
    fn persistence_resume_and_fork_reconstruct_messages() {
        let persistence = Persistence::new(temp_dir("resume"));
        let session_id = uuid::Uuid::new_v4();
        let mut thread = thread("deepseek-chat");
        let first = Message::text(MessageRole::User, "hello");
        thread.message_count = 1;
        persistence
            .record_message(&session_id, &thread, &first)
            .unwrap();

        let snapshot = persistence.load_snapshot(&session_id).unwrap().unwrap();
        assert_eq!(snapshot.session_id, session_id);
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].role, MessageRole::User);

        let fork = persistence.fork_session(&session_id).unwrap().unwrap();
        assert_ne!(fork.session_id, session_id);
        assert_ne!(fork.thread.id, snapshot.thread.id);
        assert_eq!(fork.messages.len(), 1);

        let original = persistence.load_snapshot(&session_id).unwrap().unwrap();
        assert_eq!(original.thread.id, snapshot.thread.id);
    }
}
