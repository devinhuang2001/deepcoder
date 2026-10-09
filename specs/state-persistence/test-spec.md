# State Persistence Test Spec

## Unit Tests

- `jsonl_append_event_writes_one_line`
  - 准备：临时 JSONL 路径和 JSON event。
  - 执行：`append_event`。
  - 断言：文件存在，只有一行合法 JSON。
  - Mock：临时目录。
- `session_index_load_missing_returns_default`
  - 准备：不存在的 index path。
  - 执行：`SessionIndex::load`。
  - 断言：sessions 为空。
  - Mock：临时目录。
- `session_index_save_and_load`
  - 准备：一个 session entry。
  - 执行：save 后 load。
  - 断言：entry 字段保持一致。
  - Mock：临时目录。
- `session_log_path_contains_date`
  - 准备：Persistence data dir 和 session id。
  - 执行：`session_log_path`。
  - 断言：路径在 sessions 目录，文件名包含 session id 和当前日期。
  - Mock：临时目录。

## Integration Tests

- `engine_records_turn_events`
  - 准备：启用 persistence，mock provider 返回文本。
  - 执行：run_turn。
  - 断言：JSONL 包含 turn_start/text_delta/turn_complete。
  - Mock：mock provider、临时目录。
- `engine_records_tool_events`
  - 准备：mock provider 请求工具，注册 mock tool。
  - 执行：run_turn。
  - 断言：JSONL 包含 tool_call 和 tool_result。
  - Mock：mock provider/tool。
- `resume_reconstructs_session`
  - 准备：写入 JSONL 和 index。
  - 执行：resume session。
  - 断言：messages、thread metadata 恢复。
  - Mock：临时目录。

## CLI/TUI Acceptance Tests

- `tui_session_picker_lists_sessions`
  - 准备：临时 data dir 有多个 session index entries。
  - 执行：打开 SessionPicker。
  - 断言：按更新时间倒序显示。
  - Mock：临时目录。

## Failure Cases

- `jsonl_write_failure_warns`
  - 准备：只读或不可写目录。
  - 执行：记录事件。
  - 断言：返回/记录 warning，不导致 turn panic。
  - Mock：临时目录权限。
- `corrupt_index_loads_default_with_warning`
  - 准备：非法 TOML index。
  - 执行：load index。
  - 断言：返回 default 或错误策略明确，不 panic。
  - Mock：临时目录。
