# Context Management Test Spec

## Unit Tests

- `context_budget_counts_sections`
  - 准备：system prompt、tools、skills、history。
  - 执行：计算 context budget。
  - 断言：各部分 token/estimate 均计入总数。
  - Mock：mock tokenizer。
- `context_excludes_reasoning`
  - 准备：history 包含 reasoning content。
  - 执行：构建 provider-visible context。
  - 断言：reasoning 不作为普通文本发送。
  - Mock：不需要。
- `context_keeps_recent_messages`
  - 准备：超过预算的历史。
  - 执行：compact。
  - 断言：最近 N 条消息保留 verbatim。
  - Mock：mock summarizer。

## Integration Tests

- `context_compacts_when_over_budget`
  - 准备：mock tokenizer 返回超预算。
  - 执行：run_turn 前构建 request。
  - 断言：生成摘要，request 在预算内。
  - Mock：mock tokenizer、mock summarizer、mock provider。
- `context_preserves_tool_metadata`
  - 准备：历史包含 tool call/result。
  - 执行：compact。
  - 断言：摘要包含工具名、文件路径、错误摘要或关键输出。
  - Mock：mock summarizer。
- `resume_rebuilds_context_order`
  - 准备：持久化 session 含 summary 和 recent messages。
  - 执行：resume 后构建 request。
  - 断言：summary 在 recent messages 前，顺序稳定。
  - Mock：临时 persistence。

## CLI/TUI Acceptance Tests

- `status_shows_context_usage`
  - 准备：TUI session with token usage。
  - 执行：渲染状态行。
  - 断言：显示 context/token usage。
  - Mock：TUI test backend。

## Failure Cases

- `summary_failure_falls_back_safely`
  - 准备：summarizer 返回错误。
  - 执行：compact。
  - 断言：返回明确错误或采用截断策略，不发送超预算请求。
  - Mock：mock summarizer。
- `single_message_over_budget`
  - 准备：单条用户消息超过预算。
  - 执行：构建 request。
  - 断言：返回 context error，提示用户缩短输入。
  - Mock：mock tokenizer。
