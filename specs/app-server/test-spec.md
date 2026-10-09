# AppServer Test Spec

## Unit Tests

- `jsonrpc_success_response_shape`
  - 准备：id 和 result JSON。
  - 执行：构造 success response 并序列化。
  - 断言：包含 `jsonrpc:"2.0"`、id、result，不包含 error。
  - Mock：不需要。
- `jsonrpc_error_response_shape`
  - 准备：id、code、message。
  - 执行：构造 error response。
  - 断言：包含 error.code/message，不包含 result。
  - Mock：不需要。
- `message_processor_unknown_method`
  - 准备：未注册 method。
  - 执行：处理请求。
  - 断言：返回 `-32601`。
  - Mock：mock processor registry。
- `turn_cancel_records_cancel_request`
  - 准备：JSON-RPC `turn/cancel` 请求。
  - 执行：处理请求。
  - 断言：返回 `cancelled=true`；缺少 `turn_id` 返回 `-32602`。
  - Mock：不需要。

## Integration Tests

- `websocket_handshake_enforces_origin_and_token`
  - 准备：显式安全配置、Origin 和强令牌。
  - 执行：分别用缺令牌与正确令牌握手。
  - 断言：缺令牌返回 401；正确请求返回 101 且只回显 `deepcoder-v1`。
  - Mock：本地 WebSocket listener。
- `websocket_rate_limit_closes_with_policy_violation`
  - 准备：建立合法连接。
  - 执行：在滚动窗口内发送超限 JSON-RPC 请求。
  - 断言：服务端发送 close 1008。
  - Mock：本地 WebSocket listener。
- `websocket_rejects_binary_json_rpc_frames_with_close_1003`
  - 准备：建立合法连接。
  - 执行：发送 binary frame。
  - 断言：服务端发送 close 1003，不宽松解码。
  - Mock：本地 WebSocket listener。

- `stdio_transport_round_trip`
  - 准备：启动 AppServer stdio。
  - 执行：发送 JSON-RPC request。
  - 断言：stdout 返回合法 response。
  - Mock：mock engine。
- `websocket_transport_round_trip`
  - 准备：启动 WebSocket AppServer。
  - 执行：客户端连接并发送 request。
  - 断言：收到 response，id 匹配。
  - Mock：mock engine。
- `thread_crud_api`
  - 准备：临时 persistence dir。
  - 执行：thread/create、thread/list、thread/get、thread/delete。
  - 断言：返回元数据正确，删除后不可 get。
  - Mock：临时目录。
- `thread_get_returns_persisted_messages`
  - 准备：临时 persistence dir；mock provider 返回一段 assistant text delta。
  - 执行：`thread/create` 后调用 `turn/start` 写入 user/assistant 消息，再调用 `thread/get`。
  - 断言：response 同时包含 `thread` 和 `messages`；messages 中包含用户输入和助手文本，顺序来自 JSONL 持久化。
  - Mock：mock DeepSeek SSE，不依赖真实 API key。
- `turn_start_streams_events`
  - 准备：mock provider 返回 deltas。
  - 执行：RPC start turn。
  - 断言：client 收到 text/reasoning/complete events。
  - Mock：mock provider。
- `turn_start_rejects_pre_cancelled_turn_id`
  - 准备：先调用 `turn/cancel` 记录 turn id。
  - 执行：用相同 `turn_id` 调用 `turn/start`。
  - 断言：返回 cancelled error，不调用 provider。
  - Mock：临时 persistence。
- `websocket_turn_cancel_aborts_running_turn`
  - 准备：启动 WebSocket AppServer；provider base_url 指向会接收请求但不返回的 mock HTTP server。
  - 执行：`thread/create` 后发送带 `turn_id` 的 `turn/start`；确认 mock provider 已收到请求后发送同一 `turn_id` 的 `turn/cancel`。
  - 断言：原 `turn/start` response 返回 cancellation error；`turn/cancel` response 返回 `cancelled=true` 与 `running=true`；客户端收到 `turn/event` 且 `params.type=turn_cancelled`。
  - Mock：mock hanging HTTP provider，不依赖真实 API key。

## CLI/TUI Acceptance Tests

- `app_server_cli_binds_ws`
  - 准备：可用端口。
  - 执行：`deepcoder app-server --ws <addr>`。
  - 断言：WebSocket 可连接。
  - Mock：不需要。
- `tui_can_use_inprocess_appserver`
  - 准备：TUI app with in-process client。
  - 执行：提交 query。
  - 断言：事件通过 in-process channel 到达 UI。
  - Mock：mock engine。

## Failure Cases

- `invalid_json_returns_parse_error`
  - 准备：发送非法 JSON。
  - 执行：transport read/process。
  - 断言：返回 JSON-RPC parse error。
  - Mock：不需要。
- `invalid_params_returns_error`
  - 准备：method 存在但 params 类型错误。
  - 执行：处理请求。
  - 断言：返回 `-32602`。
  - Mock：mock processor。
