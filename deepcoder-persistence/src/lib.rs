//! DeepCoder 持久化系统
//!
//! JSONL 事件日志 + TOML session 索引

pub mod jsonl;
pub mod index;

use std::path::PathBuf;

/// 持久化管理器
pub struct Persistence {
    data_dir: PathBuf,
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
        self.data_dir.join("sessions").join(format!("{session_id}_{date}.jsonl"))
    }

    /// 获取 session 索引路径
    pub fn index_path(&self) -> PathBuf {
        self.data_dir.join("sessions.toml")
    }
}
