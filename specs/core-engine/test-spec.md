# Core Engine Test Spec

## Unit Tests

- `session_new_initializes_thread`
  - 准备：默认 config 和空 ToolRouter。
  - 执行：创建 `Session`。
  - 断言：session id/thread id 非空，model 来自 config，messages 为空。
  - Mock：不需要。
- `session_add_message_updates_count`
  - 准备：新 session。
  - 执行：添加 user message。
  - 断言：message_count 增加，updated_at 更新，messages 包含消息。
  - Mock：不需要。
- `engine_filters_reasoning_from_provider_history`
  - 准备：session 中包含 reasoning content。
  - 执行：构建 provider request。
  - 断言：provider-visible messages 不包含 reasoning content。
  - Mock：mock provider。

## Integration Tests

- `engine_basic_turn_completes`
  - 准备：mock provider 返回 text delta 和 done。
  - 执行：`run_turn`。
  - 断言：发送 TurnStart/TextDelta/TurnComplete，session 保存 assistant 文本。
  - Mock：mock provider，不访问 DeepSeek。
- `engine_tool_loop_continues_after_tool_result`
  - 准备：mock provider 第一轮返回 tool_call，第二轮返回最终文本；注册 mock tool。
  - 执行：`run_turn`。
  - 断言：工具被调用一次，tool result 回灌，最终 assistant 文本保存。
  - Mock：mock provider、mock tool。
- `engine_max_tool_iterations`
  - 准备：mock provider 持续返回 tool_call，max_tool_iterations 设置为 2。
  - 执行：`run_turn`。
  - 断言：返回 Turn 错误，工具调用次数不超过限制。
  - Mock：mock provider、mock tool。
- `engine_tool_error_is_recorded`
  - 准备：注册返回错误的 mock tool。
  - 执行：模型请求该工具。
  - 断言：session 有 ToolResult 且 `is_error=true`，事件流包含 ToolResult。
  - Mock：mock provider、mock tool。

## CLI/TUI Acceptance Tests

- `exec_prints_final_answer`
  - 准备：CLI 使用 mock provider 返回最终文本。
  - 执行：`deepcoder exec "explain"`。
  - 断言：stdout 只包含最终助手回答，退出码 0。
  - Mock：mock provider。
- `tui_submit_displays_error`
  - 准备：TUI 使用无 API key 或 mock provider 错误。
  - 执行：输入文本并提交。
  - 断言：错误出现在 chat/status，输入恢复可用。
  - Mock：可用 mock provider。

## Failure Cases

- `engine_missing_api_key`
  - 准备：config 无 api_key 且环境变量不存在。
  - 执行：`run_turn`。
  - 断言：返回 Config 错误。
  - Mock：不需要。
- `engine_unknown_tool`
  - 准备：mock provider 请求未注册工具。
  - 执行：`run_turn`。
  - 断言：ToolResult error 被记录，流程不 panic。
  - Mock：mock provider。
