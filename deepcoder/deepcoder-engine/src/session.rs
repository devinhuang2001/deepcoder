//! 会话管理

use deepcoder_error::DeepCoderResult;
use deepcoder_persistence::{Persistence, SessionSnapshot};
use deepcoder_types::message::*;
use deepcoder_types::session::*;
use std::sync::Arc;
use uuid::Uuid;

/// 会话
pub struct Session {
    pub id: Uuid,
    pub thread: Thread,
    pub config: deepcoder_config::Config,
    pub messages: Vec<Message>,
    pub tool_router: Arc<deepcoder_tools::ToolRouter>,
    pub tool_approver: Option<Arc<dyn deepcoder_tools::traits::ToolApprover>>,
}

impl Session {
    pub fn new(
        config: deepcoder_config::Config,
        tool_router: Arc<deepcoder_tools::ToolRouter>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            thread: Thread {
                id: Uuid::new_v4(),
                model: config.provider.model.clone(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                message_count: 0,
                token_usage: TokenUsage::default(),
                metadata: std::collections::HashMap::new(),
            },
            config,
            messages: Vec::new(),
            tool_router,
            tool_approver: None,
        }
    }

    pub fn from_snapshot(
        config: deepcoder_config::Config,
        tool_router: Arc<deepcoder_tools::ToolRouter>,
        snapshot: SessionSnapshot,
    ) -> Self {
        Self {
            id: snapshot.session_id,
            thread: snapshot.thread,
            config,
            messages: snapshot.messages,
            tool_router,
            tool_approver: None,
        }
    }

    pub fn set_tool_approver(
        &mut self,
        approver: Option<Arc<dyn deepcoder_tools::traits::ToolApprover>>,
    ) {
        self.tool_approver = approver;
    }

    pub fn resume(
        config: deepcoder_config::Config,
        tool_router: Arc<deepcoder_tools::ToolRouter>,
        persistence: &Persistence,
        session_id: Uuid,
    ) -> DeepCoderResult<Option<Self>> {
        Ok(persistence
            .load_snapshot(&session_id)?
            .map(|snapshot| Self::from_snapshot(config, tool_router, snapshot)))
    }

    pub fn fork_from(
        config: deepcoder_config::Config,
        tool_router: Arc<deepcoder_tools::ToolRouter>,
        persistence: &Persistence,
        session_id: Uuid,
    ) -> DeepCoderResult<Option<Self>> {
        Ok(persistence
            .fork_session(&session_id)?
            .map(|snapshot| Self::from_snapshot(config, tool_router, snapshot)))
    }

    /// 添加消息到历史
    pub fn add_message(&mut self, msg: Message) {
        self.thread.message_count += 1;
        self.thread.updated_at = chrono::Utc::now();
        self.messages.push(msg);
    }

    /// 获取当前 token 使用统计
    pub fn token_usage(&self) -> &TokenUsage {
        &self.thread.token_usage
    }

    /// 获取可见消息列表（排除推理 token）
    pub fn visible_messages(&self) -> Vec<&Message> {
        self.messages
            .iter()
            .filter(|m| !matches!(m.role, MessageRole::Tool))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepcoder_persistence::Persistence;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("deepcoder_session_{name}_{}", std::process::id()));
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn config(name: &str) -> deepcoder_config::Config {
        let mut config = deepcoder_config::Config::load_default().unwrap();
        config.system.data_dir = temp_dir(name);
        config
    }

    #[test]
    fn session_new_initializes_thread() {
        let config = config("new");
        let session = Session::new(config.clone(), Arc::new(deepcoder_tools::ToolRouter::new()));
        assert_eq!(session.thread.model, config.provider.model);
        assert!(session.messages.is_empty());
        assert_ne!(session.id, uuid::Uuid::nil());
        assert_ne!(session.thread.id, uuid::Uuid::nil());
    }

    #[test]
    fn session_add_message_updates_count() {
        let config = config("add_message");
        let mut session = Session::new(config, Arc::new(deepcoder_tools::ToolRouter::new()));
        let before = session.thread.updated_at;
        session.add_message(Message::text(MessageRole::User, "hello"));
        assert_eq!(session.thread.message_count, 1);
        assert_eq!(session.messages.len(), 1);
        assert!(session.thread.updated_at >= before);
    }

    #[test]
    fn resume_and_fork_restore_usable_sessions() {
        let config = config("resume");
        let persistence = Persistence::new(config.system.data_dir.clone());
        let router = Arc::new(deepcoder_tools::ToolRouter::new());
        let mut session = Session::new(config.clone(), router.clone());
        let message = Message::text(MessageRole::User, "restore me");
        session.add_message(message.clone());
        persistence
            .record_message(&session.id, &session.thread, &message)
            .unwrap();

        let resumed = Session::resume(config.clone(), router.clone(), &persistence, session.id)
            .unwrap()
            .unwrap();
        assert_eq!(resumed.id, session.id);
        assert_eq!(resumed.thread.id, session.thread.id);
        assert_eq!(resumed.messages.len(), 1);

        let forked = Session::fork_from(config, router, &persistence, session.id)
            .unwrap()
            .unwrap();
        assert_ne!(forked.id, session.id);
        assert_ne!(forked.thread.id, session.thread.id);
        assert_eq!(forked.messages.len(), 1);
        assert_eq!(
            forked.thread.metadata.get("forked_from"),
            Some(&session.id.to_string())
        );
    }
}
