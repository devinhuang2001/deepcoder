# Desktop Interface Test Spec

## Unit Tests

- `config_wizard_required_when_key_missing`
  - 准备：config 无 `api_key`，环境变量为空。
  - 执行：调用 wizard 判断逻辑。
  - 断言：返回 true；提供 env key 时返回 false。
  - Mock：不需要。
- `save_wizard_config_updates_file_and_current_config`
  - 准备：临时 config path，填写 api_key/model/base_url。
  - 执行：保存配置向导。
  - 断言：配置文件写入，当前 app config 立即刷新，wizard 关闭。
  - Mock：临时目录。
- `reducer_applies_stream_events`
  - 准备：DesktopAppState 和一组 EngineEvent。
  - 执行：应用 TextDelta、ReasoningDelta、TurnComplete。
  - 断言：聊天消息、reasoning、token usage、streaming 状态正确。
  - Mock：不需要。
- `approval_flow_returns_user_decision`
  - 准备：DesktopApprovalPrompt with oneshot。
  - 执行：queue 后 approve。
  - 断言：waiting receiver 得到 true，工具活动记录为 approved。
  - Mock：oneshot channel。
- `desktop_approver_round_trips_decision`
  - 准备：DesktopApprover 和 runtime event channel。
  - 执行：发起 ToolApprovalRequest，测试侧批准。
  - 断言：approve future 返回 true。
  - Mock：channel。

## Integration Tests

- `spawn_turn_streams_mock_provider_events`
  - 准备：本地 mock DeepSeek SSE HTTP server。
  - 执行：通过 desktop runner 启动真实 engine turn。
  - 断言：收到 reasoning delta、text delta、turn finished；server 收到 `/chat/completions` 请求。
  - Mock：本地 HTTP/SSE。

## CLI/TUI Acceptance Tests

- `desktop_window_starts`
  - 准备：构建 `deepcoder-desktop`。
  - 执行：启动 binary。
  - 断言：出现标题为 `DeepCoder` 的桌面窗口。
  - Mock：人工/窗口自动化验证。
- `launcher_starts_desktop`
  - 准备：项目根目录。
  - 执行：双击或运行 `启动DeepCoder.bat`。
  - 断言：优先启动根目录 exe；缺失时 fallback 到 release exe 或 cargo run。
  - Mock：脚本 dry-run/人工验证。

## Failure Cases

- `wizard_rejects_empty_api_key`
  - 准备：API Key 输入为空。
  - 执行：保存配置。
  - 断言：显示错误，不关闭 wizard。
  - Mock：不需要。
- `turn_error_restores_input`
  - 准备：mock provider 返回错误。
  - 执行：提交一轮。
  - 断言：聊天区显示错误，streaming=false。
  - Mock：mock HTTP/SSE。
