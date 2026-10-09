//! Task and agent tools.

use async_trait::async_trait;
use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_types::tool::{JsonToolOutput, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{Tool, ToolContext};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskRecord {
    id: String,
    title: String,
    status: String,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Default)]
pub struct TaskStore {
    inner: Arc<Mutex<HashMap<String, TaskRecord>>>,
}

pub struct AgentTool {
    store: TaskStore,
}

pub struct TaskCreateTool {
    store: TaskStore,
}

pub struct TaskListTool {
    store: TaskStore,
}

pub struct TaskUpdateTool {
    store: TaskStore,
}

pub struct TaskDeleteTool {
    store: TaskStore,
}

impl AgentTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

impl TaskCreateTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

impl TaskListTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

impl TaskUpdateTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

impl TaskDeleteTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Create an isolated agent task contract for delegated coding work.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task": {"type": "string"},
                    "context": {"type": "string"},
                    "max_turns": {"type": "integer", "minimum": 1}
                },
                "required": ["task"],
                "additionalProperties": false
            }),
        }
    }

    async fn call(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let task = required_string(&params, "task")?;
        let context = params
            .get("context")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);
        let max_turns = params
            .get("max_turns")
            .and_then(|value| value.as_u64())
            .unwrap_or(1);
        let agent_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let now = timestamp();
        let record = TaskRecord {
            id: task_id.clone(),
            title: task.clone(),
            status: "queued".into(),
            notes: context.clone(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.store
            .inner
            .lock()
            .await
            .insert(task_id.clone(), record);

        Ok(JsonToolOutput::success(json!({
            "agent_id": agent_id,
            "agent_session_id": agent_id,
            "task_id": task_id,
            "task": task,
            "context": context,
            "max_turns": max_turns,
            "status": "queued",
            "isolated": true
        })))
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &'static str {
        "task_create"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Create a local task record.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "status": {"type": "string"},
                    "notes": {"type": "string"}
                },
                "required": ["title"],
                "additionalProperties": false
            }),
        }
    }

    async fn call(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let title = required_string(&params, "title")?;
        let status = params
            .get("status")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("todo")
            .to_string();
        let notes = params
            .get("notes")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);
        let now = timestamp();
        let record = TaskRecord {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            status,
            notes,
            created_at: now.clone(),
            updated_at: now,
        };
        self.store
            .inner
            .lock()
            .await
            .insert(record.id.clone(), record.clone());
        Ok(JsonToolOutput::success(json!({ "task": record })))
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &'static str {
        "task_list"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "List local task records, optionally filtered by status.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": {"type": "string"}
                },
                "additionalProperties": false
            }),
        }
    }

    async fn call(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let status_filter = params.get("status").and_then(|value| value.as_str());
        let mut tasks: Vec<_> = self
            .store
            .inner
            .lock()
            .await
            .values()
            .filter(|task| {
                status_filter
                    .map(|status| task.status == status)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        tasks.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(JsonToolOutput::success(json!({ "tasks": tasks })))
    }
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &'static str {
        "task_update"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Update title, status, or notes for a local task record.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "title": {"type": "string"},
                    "status": {"type": "string"},
                    "notes": {"type": "string"}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        }
    }

    async fn call(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let id = required_string(&params, "id")?;
        let mut tasks = self.store.inner.lock().await;
        let Some(task) = tasks.get_mut(&id) else {
            return Err(DeepCoderError::ToolExecution(format!(
                "task not found: {id}"
            )));
        };
        if let Some(title) = optional_non_empty_string(&params, "title") {
            task.title = title;
        }
        if let Some(status) = optional_non_empty_string(&params, "status") {
            task.status = status;
        }
        if params.get("notes").is_some() {
            task.notes = params
                .get("notes")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
        }
        task.updated_at = timestamp();
        Ok(JsonToolOutput::success(json!({ "task": task.clone() })))
    }
}

#[async_trait]
impl Tool for TaskDeleteTool {
    fn name(&self) -> &'static str {
        "task_delete"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Delete a local task record.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        }
    }

    async fn call(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let id = required_string(&params, "id")?;
        let deleted = self.store.inner.lock().await.remove(&id).is_some();
        Ok(JsonToolOutput::success(json!({
            "id": id,
            "deleted": deleted
        })))
    }
}

fn required_string(params: &serde_json::Value, key: &str) -> DeepCoderResult<String> {
    params
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| DeepCoderError::ToolExecution(format!("missing required string: {key}")))
}

fn optional_non_empty_string(params: &serde_json::Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}
