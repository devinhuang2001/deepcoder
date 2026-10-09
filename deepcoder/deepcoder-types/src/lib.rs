//! DeepCoder 共享类型定义

pub mod event;
pub mod message;
pub mod provider;
pub mod session;
pub mod tool;

use uuid::Uuid;

/// 核心类型别名
pub type ThreadId = Uuid;
pub type SessionId = Uuid;
pub type TurnId = Uuid;
pub type MessageId = Uuid;
pub type ToolCallId = String;
