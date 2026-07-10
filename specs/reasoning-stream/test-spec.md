# Reasoning Stream Test Spec

## Unit Tests

- `reasoning_delta_accumulates`
  - 准备：两个 reasoning delta。
  - 执行：engine 处理事件。
  - 断言：turn reasoning content 为拼接结果。
  - Mock：mock provider。
- `reasoning_saved_as_content_type`
  - 准备：turn 包含 reasoning。
  - 执行：turn 完成。
  - 断言：session assistant/reasoning message 中存在 `ContentType::Reasoning`。
  - Mock：mock provider。
- `reasoning_not_sent_back`
  - 准备：session 历史包含 reasoning。
  - 执行：下一轮 provider request。
  - 断言：request messages 不含 reasoning text。
  - Mock：mock provider。

## Integration Tests

- `provider_to_tui_reasoning_flow`
  - 准备：mock provider SSE 返回 reasoning 和 text。
  - 执行：TUI/App state 处理事件。
  - 断言：reasoning 面板内容更新，chat 文本独立更新。
  - Mock：mock provider。
- `reasoning_jsonl_persistence`
  - 准备：启用 persistence。
  - 执行：run_turn 收到 reasoning delta。
  - 断言：JSONL 中有 reasoning event。
  - Mock：mock provider、临时目录。

## CLI/TUI Acceptance Tests

- `tui_can_hide_reasoning`
  - 准备：show_reasoning=false。
  - 执行：收到 reasoning delta。
  - 断言：reasoning 不渲染，answer 仍渲染。
  - Mock：mock event stream。

## Failure Cases

- `reasoning_empty_delta_ignored`
  - 准备：SSE reasoning_content 为空字符串。
  - 执行：读取 provider event。
  - 断言：不产生空 reasoning event。
  - Mock：mock SSE。
