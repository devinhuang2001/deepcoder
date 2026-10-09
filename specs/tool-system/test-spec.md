# Tool System Test Spec

## Unit Tests

- `tool_router_register_and_execute`
  - 准备：注册 mock tool。
  - 执行：调用 Router `execute`。
  - 断言：返回 mock output，参数原样传入。
  - Mock：mock tool。
- `tool_router_unknown_tool`
  - 准备：空 Router。
  - 执行：调用不存在工具。
  - 断言：返回 `ToolNotFound`。
  - Mock：不需要。
- `tool_router_permission_denied`
  - 准备：注册 permission deny 的 mock tool。
  - 执行：调用工具。
  - 断言：返回 `ToolDenied`，mock tool call 未执行。
  - Mock：mock tool。
- `file_read_success`
  - 准备：临时文本文件。
  - 执行：`read_file`。
  - 断言：返回内容和行号。
  - Mock：临时目录。
- `file_write_and_edit`
  - 准备：临时目录。
  - 执行：`write_file` 后 `edit_file`。
  - 断言：文件内容变更，edit output 包含 diff。
  - Mock：临时目录。
- `bash_success`
  - 准备：允许 `echo` 的 policy。
  - 执行：`bash` 运行 `echo hello`。
  - 断言：exit code 0，stdout 包含 hello。
  - Mock：不需要。
- `bash_prompt_requires_explicit_approval`
  - 准备：未知前缀安全命令和 mock approver。
  - 执行：无 approver 调用一次，再用 approving approver 调用一次。
  - 断言：无 approver 返回 `ToolDenied`；批准后命令执行成功。
  - Mock：mock approver。
- `bash_uses_configured_sandbox_mode`
  - 准备：同一条无害 echo 命令包含危险模式文本；分别设置 sandbox `enforce` 与 `off`。
  - 执行：调用 `bash`。
  - 断言：`enforce` 返回 `ToolDenied`，`off` 可执行。
  - Mock：不执行真实危险命令。
- `glob_and_grep`
  - 准备：临时文件树。
  - 执行：`glob` 与 `grep`。
  - 断言：返回匹配路径、行号、文本。
  - Mock：临时目录。

## Integration Tests

- `builtin_tools_registered`
  - 准备：调用内置工具注册函数。
  - 执行：读取 Router direct specs。
  - 断言：包含 read/write/edit/bash/glob/grep/web_fetch/web_search/agent/task_create/task_list/task_update/task_delete。
  - Mock：不需要。
- `task_tools_create_list_update_and_delete`
  - 准备：使用同一个内置 Router。
  - 执行：task_create、task_list、task_update、task_delete。
  - 断言：任务状态可更新，过滤列表正确，删除后列表为空。
  - Mock：内存 store。
- `agent_tool_spawns_isolated_task_contract`
  - 准备：使用内置 Router。
  - 执行：调用 `agent`。
  - 断言：返回独立 `agent_session_id`、queued 状态，并写入任务列表。
  - Mock：内存 store，不调用真实 provider。
- `bash_respects_sandbox_policy`
  - 准备：sandbox mode enforce，危险命令。
  - 执行：调用 bash。
  - 断言：返回 denied，不执行命令。
  - Mock：不执行危险命令，只验证 policy path。
- `web_fetch_mock_http`
  - 准备：mock HTTP server。
  - 执行：`web_fetch`。
  - 断言：返回正文/markdown 和 status metadata。
  - Mock：mock HTTP server。

## CLI/TUI Acceptance Tests

- `model_can_call_file_tool`
  - 准备：mock provider 请求 `read_file`。
  - 执行：`deepcoder exec "read file"`。
  - 断言：工具结果回灌，最终回答包含文件内容摘要。
  - Mock：mock provider、临时文件。

## Failure Cases

- `file_edit_no_match`
  - 准备：文件不包含 old_string。
  - 执行：`edit_file`。
  - 断言：返回 tool error，文件不变。
  - Mock：临时目录。
- `file_edit_multiple_match_rejected`
  - 准备：文件包含多个 old_string。
  - 执行：`edit_file`。
  - 断言：默认拒绝模糊编辑。
  - Mock：临时目录。
- `bash_timeout`
  - 准备：设置短 timeout。
  - 执行：运行长时间命令。
  - 断言：返回 timeout error，进程被终止。
  - Mock：不需要。
- `grep_invalid_regex`
  - 准备：非法正则。
  - 执行：`grep`。
  - 断言：返回结构化错误。
  - Mock：不需要。
