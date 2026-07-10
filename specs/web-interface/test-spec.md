# Web Interface Test Spec

## Unit Tests

- `message_to_bubble_renders_text_only`
  - 准备：DeepCoder `Message` JSON，包含 `Text`、`Reasoning`、`ToolCall`、`ToolResult` contents。
  - 执行：调用 Web message conversion helper。
  - 断言：聊天气泡只包含正文 text；reasoning/tool 内容不重复塞进气泡。
  - Mock：不需要。
- `message_to_bubble_omits_non_text_only_messages`
  - 准备：只包含 `ToolResult` 或 reasoning 的 message。
  - 执行：调用 Web message conversion helper。
  - 断言：返回 `null`，中央聊天区不出现空气泡。
  - Mock：不需要。
- `extract_diagnostics_from_messages`
  - 准备：包含 reasoning、tool call、tool result 的历史 messages。
  - 执行：调用 diagnostics extraction helper。
  - 断言：返回 `reasoningText`、`toolEvents`；tool result 错误态映射为失败样式。
  - Mock：不需要。
- `build_thread_transcript_restores_visible_messages`
  - 准备：thread id 和 user/assistant/reasoning messages。
  - 执行：调用 transcript builder。
  - 断言：结果包含系统载入提示、用户消息、助手正文，不包含 reasoning-only 气泡。
  - Mock：不需要。
- `create_client_turn_id_uses_crypto_when_available`
  - 准备：提供 `crypto.randomUUID` mock。
  - 执行：调用 turn id helper。
  - 断言：优先使用 crypto 生成的 UUID。
  - Mock：mock crypto scope。
- `resolve_websocket_url_uses_secure_same_origin_endpoint_in_production`
  - 准备：HTTPS location。
  - 执行：解析默认 WebSocket URL。
  - 断言：返回同 host 的 `wss://.../ws`。
  - Mock：mock location。
- `create_websocket_protocols_keeps_token_in_memory_only`
  - 准备：合法与非法访问令牌。
  - 执行：构造 WebSocket protocols。
  - 断言：合法值包含 `deepcoder-v1` 和 token protocol；短令牌被拒绝。
  - Mock：不需要。
- `calculate_reconnect_delay_uses_capped_exponential_backoff`
  - 准备：不同 attempt 与固定 jitter。
  - 执行：计算重连延迟。
  - 断言：指数增长并封顶 30 秒。
  - Mock：mock random。
- `request_json_rpc_rejects_when_disconnected`
  - 准备：`wsRef.current` 为空或非 OPEN。
  - 执行：调用 `requestJsonRpc`。
  - 断言：Promise reject，错误提示包含 WebSocket 未连接。
  - Mock：mock WebSocket ref。
- `turn_cancel_event_clears_busy_state`
  - 准备：active assistant id、active turn id、busy state。
  - 执行：处理 `turn/event` 且 `type=turn_cancelled`。
  - 断言：assistant message 显示已停止，busy/cancelling 清空。
  - Mock：mock state reducer。

## Integration Tests

- `web_connects_and_loads_threads`
  - 准备：mock WebSocket server，按顺序响应 `initialize`、`thread/list`、`thread/get`。
  - 执行：启动 Web app。
  - 断言：左侧显示 thread，中央显示历史消息。
  - Mock：mock JSON-RPC WebSocket。
- `web_starts_turn_and_streams_text`
  - 准备：mock WebSocket server 接收 `turn/start` 后发送 `turn/event` text deltas 和 final response。
  - 执行：输入文本并发送。
  - 断言：助手气泡增量更新，完成后 busy 状态清空。
  - Mock：mock JSON-RPC WebSocket。
- `web_updates_right_panel_from_turn_events`
  - 准备：mock WebSocket server 发送 reasoning/tool/token events。
  - 执行：运行一轮 turn。
  - 断言：右侧 Reasoning、Tools、Tokens 面板展示对应内容。
  - Mock：mock JSON-RPC WebSocket。
- `web_cancel_calls_turn_cancel`
  - 准备：mock WebSocket server 让 `turn/start` 保持 pending。
  - 执行：发送后点击停止按钮。
  - 断言：客户端发送 `turn/cancel`，params 包含当前 `turn_id`。
  - Mock：mock JSON-RPC WebSocket。
- `web_launcher_validate_only`
  - 准备：项目根目录存在 `deepcoder` 和 `deepcoder-web`。
  - 执行：`powershell -ExecutionPolicy Bypass -File .\StartDeepCoderWeb.ps1 -ValidateOnly`。
  - 断言：退出码 0；不启动 AppServer/Vite；不打开浏览器。
  - Mock：不需要。
- `web_visual_smoke_screenshots`
  - 准备：启动 `deepcoder-web` Vite dev server；可不启动 AppServer，以 disconnected state 作为基线。
  - 执行：`npx playwright screenshot --channel msedge --viewport-size=1440,900 http://127.0.0.1:<port> output/playwright/deepcoder-web-desktop.png` 与 `--viewport-size=390,844`。
  - 断言：截图非空；桌面三栏清晰；移动端顶栏、消息区、输入区无明显重叠、溢出或空白断层。
  - Mock：不需要真实 API key；可使用 disconnected AppServer state。

## CLI/TUI Acceptance Tests

- `one_click_web_launcher_opens_browser`
  - 准备：Windows、有 cargo/npm 或已有 release binary/node_modules。
  - 执行：双击 `启动DeepCoder网页版.bat`。
  - 断言：`http://127.0.0.1:5173` 可访问，AppServer `127.0.0.1:8080` 可连接。
  - Mock：可使用本地无 API key，仅验证 UI 启动。
- `web_works_with_real_appserver_mock_provider`
  - 准备：AppServer config 指向 mock DeepSeek SSE provider。
  - 执行：通过 Web 提交 prompt。
  - 断言：Web 显示 mock assistant response、reasoning/tool/token 状态。
  - Mock：mock DeepSeek SSE。

## Failure Cases

- `web_displays_appserver_disconnected`
  - 准备：不启动 AppServer。
  - 执行：打开 Web app。
  - 断言：显示连接错误和启动提示，输入框禁用。
  - Mock：不需要。
- `web_handles_bad_json_rpc_payload`
  - 准备：WebSocket server 发送非法 JSON。
  - 执行：打开 Web app 并接收消息。
  - 断言：系统消息显示响应解析失败，不崩溃。
  - Mock：mock WebSocket。
- `web_launcher_missing_npm_reports_error`
  - 准备：PATH 中无 npm。
  - 执行：运行 `StartDeepCoderWeb.ps1`。
  - 断言：窗口保留中文/英文错误说明，不静默退出。
  - Mock：PATH override。
- `web_launcher_port_conflict_reuses_existing_service`
  - 准备：`127.0.0.1:8080` 或 `5173` 已监听。
  - 执行：运行 Web launcher。
  - 断言：脚本不重复启动对应服务，并继续打开 Web URL。
  - Mock：本地 dummy TCP listener。
