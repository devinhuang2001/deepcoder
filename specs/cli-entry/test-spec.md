# CLI Entry Test Spec

## Unit Tests

- `cli_parses_default_mode`
  - 准备：参数 `deepcoder`。
  - 执行：clap parse。
  - 断言：command 为 interactive/default。
  - Mock：不需要。
- `cli_parses_exec`
  - 准备：参数 `deepcoder exec hello`。
  - 执行：clap parse。
  - 断言：query 为 hello。
  - Mock：不需要。
- `cli_applies_model_and_api_key_overrides`
  - 准备：默认 config，CLI flags `--model m --api-key k`。
  - 执行：应用覆盖。
  - 断言：config provider model 为 m，api_key 为 k。
  - Mock：不需要。

## Integration Tests

- `exec_success_with_mock_provider`
  - 准备：mock provider 返回文本。
  - 执行：启动真实 `CARGO_BIN_EXE_deepcoder`，通过 `--config` 指向 mock provider base_url。
  - 断言：stdout 包含文本，退出码 0。
  - Mock：本地 mock HTTP/SSE provider。
- `app_server_command_binds_address`
  - 准备：可用本地端口。
  - 执行：启动 `deepcoder app-server --ws <addr>`。
  - 断言：端口可连接，进程可停止。
  - Mock：不需要。
- `mcp_server_command_starts_stdio`
  - 准备：子进程 stdin/stdout。
  - 执行：启动 `deepcoder mcp-server`。
  - 断言：进程接受 initialize 请求并返回 MCP 响应。
  - Mock：不需要。

## CLI/TUI Acceptance Tests

- `help_lists_commands`
  - 准备：无。
  - 执行：`deepcoder --help`。
  - 断言：输出包含 `exec`、`mcp-server`、`app-server`、`config`。
  - Mock：不需要。
- `missing_api_key_error_is_actionable`
  - 准备：清空 API key。
  - 执行：单元测试 preflight；二进制验收测试运行 `deepcoder --config <path> exec hi` 并移除 `DEEPSEEK_API_KEY`。
  - 断言：非零退出码，stderr 包含 `DEEPSEEK_API_KEY`。
  - Mock：不需要。

## Failure Cases

- `invalid_subcommand`
  - 准备：参数 `deepcoder unknown`。
  - 执行：CLI parse。
  - 断言：clap 返回错误，退出码非零。
  - Mock：不需要。
- `provider_error_sets_nonzero_exit`
  - 准备：mock provider 返回 500。
  - 执行：CLI exec。
  - 断言：退出码非零，stderr 包含 provider error。
  - Mock：mock provider。
