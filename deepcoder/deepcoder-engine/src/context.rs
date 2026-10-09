//! Context budgeting and compaction.

use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_types::provider::ChatMessage;
use deepcoder_types::tool::ToolSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBudget {
    pub system_tokens: u64,
    pub tool_tokens: u64,
    pub skill_tokens: u64,
    pub history_tokens: u64,
    pub total_tokens: u64,
    pub max_tokens: u64,
}

pub fn context_budget(
    system_prompt: Option<&str>,
    skill_prompt: Option<&str>,
    tools: &[ToolSpec],
    history: &[ChatMessage],
    max_tokens: u64,
) -> ContextBudget {
    let system_tokens = system_prompt.map(estimate_text_tokens).unwrap_or_default();
    let skill_tokens = skill_prompt.map(estimate_text_tokens).unwrap_or_default();
    let tool_tokens = estimate_text_tokens(&serde_json::to_string(tools).unwrap_or_default());
    let history_tokens = history.iter().map(estimate_chat_message_tokens).sum();
    ContextBudget {
        system_tokens,
        tool_tokens,
        skill_tokens,
        history_tokens,
        total_tokens: system_tokens + skill_tokens + tool_tokens + history_tokens,
        max_tokens,
    }
}

pub fn compact_chat_messages(
    messages: Vec<ChatMessage>,
    max_tokens: u64,
    keep_recent: usize,
) -> DeepCoderResult<Vec<ChatMessage>> {
    if estimate_chat_messages_tokens(&messages) <= max_tokens {
        return Ok(messages);
    }

    let (system_messages, non_system): (Vec<_>, Vec<_>) = messages
        .into_iter()
        .partition(|message| message.role == "system");
    let keep_count = keep_recent.min(non_system.len());
    let split_at = non_system.len().saturating_sub(keep_count);
    let removed = &non_system[..split_at];
    let recent = &non_system[split_at..];

    let recent_tokens =
        estimate_chat_messages_tokens(&system_messages) + estimate_chat_messages_tokens(recent);
    if recent_tokens > max_tokens {
        return Err(DeepCoderError::Turn(
            "single recent context window exceeds max_context_tokens".into(),
        ));
    }

    let remaining_for_summary = max_tokens.saturating_sub(recent_tokens);
    let summary = summarize_removed_messages(removed, remaining_for_summary);
    let mut compacted = system_messages;
    if !summary.is_empty() {
        compacted.push(ChatMessage {
            role: "system".into(),
            content: serde_json::Value::String(summary),
            tool_call_id: None,
            tool_calls: None,
        });
    }
    compacted.extend_from_slice(recent);

    while estimate_chat_messages_tokens(&compacted) > max_tokens
        && compacted.iter().any(|message| {
            message
                .content
                .as_str()
                .is_some_and(|text| !text.is_empty())
        })
    {
        if let Some(summary) = compacted.iter_mut().find(|message| {
            message.role == "system"
                && message
                    .content
                    .as_str()
                    .is_some_and(|text| text.starts_with("Context summary:"))
        }) {
            let text = summary.content.as_str().unwrap_or_default();
            let shortened = text
                .chars()
                .take(text.chars().count() / 2)
                .collect::<String>();
            summary.content = serde_json::Value::String(shortened);
        } else {
            break;
        }
    }

    Ok(compacted)
}

pub fn estimate_chat_messages_tokens(messages: &[ChatMessage]) -> u64 {
    messages.iter().map(estimate_chat_message_tokens).sum()
}

pub fn estimate_chat_message_tokens(message: &ChatMessage) -> u64 {
    estimate_value_tokens(&message.content)
        + message
            .tool_calls
            .as_ref()
            .map(|tool_calls| {
                estimate_text_tokens(&serde_json::to_string(tool_calls).unwrap_or_default())
            })
            .unwrap_or_default()
        + message
            .tool_call_id
            .as_ref()
            .map(|id| estimate_text_tokens(id))
            .unwrap_or_default()
}

pub fn estimate_value_tokens(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::String(text) => estimate_text_tokens(text),
        serde_json::Value::Null => 0,
        other => estimate_text_tokens(&other.to_string()),
    }
}

pub fn estimate_text_tokens(text: &str) -> u64 {
    let chars = text.chars().filter(|ch| !ch.is_whitespace()).count() as u64;
    if chars == 0 {
        0
    } else {
        chars.div_ceil(4).max(1)
    }
}

fn summarize_removed_messages(messages: &[ChatMessage], max_tokens: u64) -> String {
    if messages.is_empty() || max_tokens == 0 {
        return String::new();
    }

    let mut summary = String::from("Context summary:\n");
    for message in messages {
        summary.push_str("- ");
        summary.push_str(&message.role);
        summary.push_str(": ");
        if let Some(text) = message.content.as_str() {
            summary.push_str(&compact_text(text, 240));
        } else if !message.content.is_null() {
            summary.push_str(&compact_text(&message.content.to_string(), 240));
        }
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                let name = tool_call
                    .get("function")
                    .and_then(|function| function.get("name"))
                    .and_then(|name| name.as_str())
                    .unwrap_or("tool");
                summary.push_str(&format!(" tool_call={name}"));
            }
        }
        if let Some(tool_call_id) = &message.tool_call_id {
            summary.push_str(&format!(" tool_result_for={tool_call_id}"));
        }
        summary.push('\n');
        if estimate_text_tokens(&summary) >= max_tokens {
            break;
        }
    }
    while estimate_text_tokens(&summary) > max_tokens && summary.len() > 16 {
        summary.truncate(summary.len() / 2);
    }
    summary
}

fn compact_text(text: &str, max_chars: usize) -> String {
    let mut compacted = text.replace('\n', " ");
    if compacted.chars().count() > max_chars {
        compacted = compacted.chars().take(max_chars).collect::<String>();
        compacted.push_str("...");
    }
    compacted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.into(),
            content: serde_json::Value::String(content.into()),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    #[test]
    fn context_budget_counts_sections() {
        let budget = context_budget(
            Some("system prompt"),
            Some("skill prompt"),
            &[ToolSpec {
                name: "read_file".into(),
                description: "Read".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
            &[msg("user", "history")],
            1000,
        );
        assert!(budget.system_tokens > 0);
        assert!(budget.skill_tokens > 0);
        assert!(budget.tool_tokens > 0);
        assert!(budget.history_tokens > 0);
        assert_eq!(
            budget.total_tokens,
            budget.system_tokens + budget.skill_tokens + budget.tool_tokens + budget.history_tokens
        );
    }

    #[test]
    fn context_keeps_recent_messages() {
        let messages = (0..12)
            .map(|idx| {
                msg(
                    if idx % 2 == 0 { "user" } else { "assistant" },
                    &"x".repeat(80),
                )
            })
            .collect::<Vec<_>>();
        let compacted = compact_chat_messages(messages, 100, 4).unwrap();
        assert!(compacted.iter().any(|message| {
            message
                .content
                .as_str()
                .unwrap_or_default()
                .contains("Context summary")
        }));
        assert_eq!(
            compacted
                .iter()
                .rev()
                .take(4)
                .filter(|message| message.role != "system")
                .count(),
            4
        );
    }

    #[test]
    fn context_preserves_tool_metadata() {
        let mut messages = vec![msg("user", "old")];
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: serde_json::Value::Null,
            tool_call_id: None,
            tool_calls: Some(vec![serde_json::json!({
                "type": "function",
                "function": {"name": "read_file", "arguments": "{\"path\":\"src/main.rs\"}"}
            })]),
        });
        messages.push(ChatMessage {
            role: "tool".into(),
            content: serde_json::Value::String(
                serde_json::json!({"path": "src/main.rs", "content": "hello"}).to_string(),
            ),
            tool_call_id: Some("call_1".into()),
            tool_calls: None,
        });
        messages.extend((0..4).map(|idx| msg("user", &format!("recent {idx}"))));

        let compacted = compact_chat_messages(messages, 30, 2).unwrap();
        let summary = compacted
            .iter()
            .find_map(|message| message.content.as_str())
            .unwrap();
        assert!(summary.contains("read_file") || summary.contains("tool_result_for"));
    }

    #[test]
    fn single_message_over_budget() {
        let messages = vec![msg("user", &"x".repeat(2000))];
        let error = compact_chat_messages(messages, 10, 1).unwrap_err();
        assert!(error.to_string().contains("max_context_tokens"));
    }
}
