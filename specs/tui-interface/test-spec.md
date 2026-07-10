# TUI Interface Test Spec

## Unit Tests

- `app_initial_state`
  - 准备：默认 config。
  - 执行：创建 `App`。
  - 断言：messages/input/reasoning 为空，streaming=false。
  - Mock：不需要。
- `handle_key_appends_char`
  - 准备：App 初始状态。
  - 执行：发送字符 key。
  - 断言：input 增加字符。
  - Mock：不需要。
- `handle_key_backspace`
  - 准备：input 有内容。
  - 执行：Backspace。
  - 断言：删除最后一个字符。
  - Mock：不需要。
- `handle_key_supports_multiline_input`
  - 准备：App 初始状态。
  - 执行：输入字符、Shift+Enter、再输入字符。
  - 断言：input 包含换行。
  - Mock：不需要。
- `input_history_moves_up_and_down`
  - 准备：App history 包含多条输入。
  - 执行：Up/Down。
  - 断言：input 按历史顺序切换，越过末尾后清空。
  - Mock：不需要。
- `widgets_render_without_panic`
  - 准备：ratatui test backend。
  - 执行：render chat/reasoning/input/status。
  - 断言：不 panic，buffer 包含标题和文本。
  - Mock：ratatui test backend。

## Integration Tests

- `tui_streaming_updates_chat`
  - 准备：mock engine event channel。
  - 执行：发送 TextDelta。
  - 断言：chat message 追加文本。
  - Mock：mock engine events。
- `tui_reasoning_panel_updates`
  - 准备：show_reasoning=true。
  - 执行：发送 ReasoningDelta。
  - 断言：reasoning 字符串和 panel 更新。
  - Mock：mock engine events。
- `tui_approval_overlay_blocks_tool`
  - 准备：工具返回 Prompt permission。
  - 执行：触发 tool call。
  - 断言：overlay 出现，未批准前工具不执行。
  - Mock：mock tool/router。
- `tui_approver_sends_prompt_and_receives_denial`
  - 准备：TUI approver channel。
  - 执行：发送审批请求并按 `n`。
  - 断言：pending overlay 出现，oneshot 返回 false。
  - Mock：mock approval request。
- `session_picker_resumes_persisted_session`
  - 准备：临时 persistence dir 写入 session/message。
  - 执行：Ctrl+S 打开 picker，Enter resume。
  - 断言：picker 关闭，当前 session 恢复并显示消息。
  - Mock：临时 persistence。

## CLI/TUI Acceptance Tests

- `tui_start_and_exit`
  - 准备：启动 TUI。
  - 执行：发送 Esc 或 Ctrl-C。
  - 断言：进程退出，terminal 恢复。
  - Mock：terminal automation。
- `tui_submit_query`
  - 准备：mock provider 返回文本。
  - 执行：输入 query 并 Enter。
  - 断言：用户消息和助手消息显示。
  - Mock：mock provider。
- `tui_render_markdown_and_diff`
  - 准备：assistant message 包含 markdown 和 diff。
  - 执行：渲染 UI。
  - 断言：代码块、列表、diff 增删行样式可见。
  - Mock：ratatui snapshot/test backend。
- `session_picker_renders_sessions`
  - 准备：ratatui test backend 和 session entry。
  - 执行：渲染 SessionPicker。
  - 断言：buffer 包含 Sessions 和 summary。
  - Mock：ratatui test backend。

## Failure Cases

- `tui_provider_error_restores_input`
  - 准备：mock provider 返回错误。
  - 执行：提交 query。
  - 断言：错误显示，streaming=false，input 可再次输入。
  - Mock：mock provider。
- `tui_resize_no_overlap`
  - 准备：小尺寸 terminal。
  - 执行：render。
  - 断言：不 panic，不越界。
  - Mock：ratatui test backend。
