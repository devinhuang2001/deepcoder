//! DeepCoder 共享类型定义

pub mod tool;
pub mod message;
pub mod session;
pub mod provider;
pub mod event;

pub use message::Message;

use uuid::Uuid;

/// 核心类型别名
pub type ThreadId = Uuid;
pub type SessionId = Uuid;
pub type TurnId = Uuid;
pub type MessageId = Uuid;
pub type ToolCallId = String;
