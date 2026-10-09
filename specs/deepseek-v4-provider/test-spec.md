# DeepSeek V4 Provider Test Spec

## Unit Tests

- `provider_builds_request_body`
  - 准备：构造 `ChatRequest`，包含 model、messages、tools、max_tokens、temperature。
  - 执行：调用 request body 构建逻辑。
  - 断言：JSON 包含所有字段，tools 为空时不发送 tools。
  - Mock：不需要。
- `provider_parses_text_delta`
  - 准备：SSE 行 `data: {"choices":[{"delta":{"content":"hi"}}]}`。
  - 执行：读取下一事件。
  - 断言：返回 `StreamEvent::TextDelta("hi")`。
  - Mock：mock byte stream。
- `provider_parses_reasoning_delta`
  - 准备：SSE 行包含 `reasoning_content`。
  - 执行：读取下一事件。
  - 断言：返回 `ReasoningDelta`。
  - Mock：mock byte stream。
- `provider_parses_tool_call`
  - 准备：SSE 行包含 `tool_calls[0].function.name` 与 JSON arguments 字符串。
  - 执行：读取下一事件。
  - 断言：返回 id/name/arguments 完整的 tool call。
  - Mock：mock byte stream。
- `provider_parses_done`
  - 准备：SSE 行 `data: [DONE]`。
  - 执行：读取事件两次。
  - 断言：第一次为 `Done`，第二次为 `None`。
  - Mock：mock byte stream。

## Integration Tests

- `provider_posts_to_mock_base_url`
  - 准备：启动 mock HTTP server，路径 `/chat/completions` 返回 SSE。
  - 执行：使用 mock base_url 调 `chat_stream`。
  - 断言：收到 POST、Authorization header、Content-Type、请求体字段正确。
  - Mock：需要 mock HTTP/SSE，不使用真实 API key。
- `provider_streams_multiple_chunks`
  - 准备：mock server 返回 text、reasoning、tool、done 多个 chunk。
  - 执行：循环读取 stream。
  - 断言：事件顺序与输入 chunk 一致。
  - Mock：需要 mock HTTP/SSE。

## CLI/TUI Acceptance Tests

- `exec_uses_provider_stream`
  - 准备：CLI 指向 mock DeepSeek base URL，设置假 API key。
  - 执行：`deepcoder exec "hello"`。
  - 断言：stdout 包含 mock assistant 文本，进程退出码为 0。
  - Mock：需要 mock HTTP/SSE。

## Failure Cases

- `provider_non_2xx_returns_error`
  - 准备：mock server 返回 401 或 500。
  - 执行：`chat_stream`。
  - 断言：返回 provider/API 错误，错误信息包含 status。
  - Mock：需要。
- `provider_bad_json_returns_error`
  - 准备：SSE data 为非法 JSON。
  - 执行：读取事件。
  - 断言：返回序列化/provider 错误。
  - Mock：mock byte stream。
- `provider_network_failure_returns_error`
  - 准备：base URL 指向不可连接端口。
  - 执行：`chat_stream`。
  - 断言：返回 API 错误，不 panic。
  - Mock：不需要真实 API。
