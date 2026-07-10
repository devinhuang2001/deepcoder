# Sandbox Security Test Spec

## Unit Tests

- `exec_policy_allows_known_prefix`
  - 准备：默认 `ExecPolicy`。
  - 执行：evaluate `cargo test`。
  - 断言：返回 Allow。
  - Mock：不需要。
- `exec_policy_prompts_unknown_prefix`
  - 准备：默认 `ExecPolicy`。
  - 执行：evaluate `python script.py`。
  - 断言：返回 Prompt。
  - Mock：不需要。
- `exec_policy_custom_deny`
  - 准备：添加 deny rule。
  - 执行：evaluate matching command。
  - 断言：返回 Deny。
  - Mock：不需要。
- `sandbox_manager_rejects_dangerous_pattern`
  - 准备：sandbox enabled。
  - 执行：check command containing `rm -rf /`。
  - 断言：返回 Err。
  - Mock：不执行命令。
- `sandbox_auto_reports_platform_backend`
  - 准备：当前运行平台。
  - 执行：创建 `SandboxManager::new("auto")`。
  - 断言：backend 类型与平台检测一致，enabled 与 backend 可用性一致。
  - Mock：不需要。

## Integration Tests

- `bash_tool_blocks_denied_command`
  - 准备：BashTool with deny policy。
  - 执行：调用被拒命令。
  - 断言：命令不执行，返回 denied tool output。
  - Mock：mock command runner 或无害命令。
- `bash_uses_configured_sandbox_mode`
  - 准备：同一条无害 echo 命令包含危险模式文本；分别设置 sandbox `enforce` 与 `off`。
  - 执行：调用 BashTool。
  - 断言：`enforce` fail-closed，`off` 允许。
  - Mock：不执行真实危险命令。
- `process_hardening_removes_dynamic_linker_env`
  - 准备：设置平台对应危险 env。
  - 执行：apply_hardening。
  - 断言：env 被移除。
  - Mock：环境变量隔离。

## CLI/TUI Acceptance Tests

- `approval_overlay_for_prompt_policy`
  - 准备：shell command policy returns Prompt。
  - 执行：TUI 触发 BashTool。
  - 断言：审批弹窗出现；拒绝后命令不执行，批准后执行。
  - Mock：mock BashTool/runner。

## Failure Cases

- `bash_timeout_kills_process`
  - 准备：短 timeout。
  - 执行：运行长命令。
  - 断言：返回 timeout，子进程终止。
  - Mock：使用安全 sleep 命令。
- `sandbox_error_propagates_to_tool_result`
  - 准备：platform backend 返回 error。
  - 执行：BashTool。
  - 断言：ToolResult `is_error=true`，错误信息清晰。
  - Mock：mock backend。
