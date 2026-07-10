# MCP Integration Test Spec

## Unit Tests

- `mcp_tool_spec_conversion`
  - 准备：MCP tool schema。
  - 执行：转换为 DeepCoder `ToolSpec`。
  - 断言：name、description、input_schema 保留。
  - Mock：不需要。
- `deepcoder_tool_to_mcp_tool`
  - 准备：注册 mock DeepCoder tool。
  - 执行：生成 MCP tools/list item。
  - 断言：schema 与 DeepCoder ToolSpec 一致。
  - Mock：mock tool。

## Integration Tests

- `mcp_client_initialize_and_list_tools`
  - 准备：mock MCP server。
  - 执行：client initialize 和 tools/list。
  - 断言：server capabilities 记录，工具注册进 ToolRouter。
  - Mock：mock MCP server。
- `mcp_client_calls_tool`
  - 准备：mock MCP server 返回 tool result。
  - 执行：调用包装后的工具。
  - 断言：返回 `JsonToolOutput`。
  - Mock：mock MCP server。
- `mcp_server_tools_list`
  - 准备：DeepCoder 注册 mock tool。
  - 执行：外部 MCP client 请求 tools/list。
  - 断言：返回 mock tool。
  - Mock：mock MCP client。
- `mcp_server_tools_call`
  - 准备：DeepCoder 注册 mock tool。
  - 执行：外部 MCP client 请求 tools/call。
  - 断言：tool 执行，MCP response 包含结果。
  - Mock：mock MCP client/tool。

## CLI/TUI Acceptance Tests

- `mcp_server_cli_protocol_smoke`
  - 准备：启动 `deepcoder mcp-server` 子进程。
  - 执行：发送 initialize 和 tools/list。
  - 断言：stdout 返回 MCP compliant response。
  - Mock：不需要。

## Failure Cases

- `mcp_unknown_tool`
  - 准备：MCP tools/call unknown name。
  - 执行：server 处理。
  - 断言：返回 tool-not-found 错误。
  - Mock：mock client。
- `mcp_server_disconnect`
  - 准备：client 断开。
  - 执行：MCP client read loop。
  - 断言：返回可恢复错误，不 panic。
  - Mock：mock MCP server。
