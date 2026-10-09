# Extension System Test Spec

## Unit Tests

- `registry_builder_empty`
  - 准备：新 builder。
  - 执行：build。
  - 断言：registry 各 contributor 列表为空。
  - Mock：不需要。
- `registry_builder_adds_contributors`
  - 准备：mock tool provider、prompt contributor、turn hook。
  - 执行：with_* 后 build。
  - 断言：registry 返回对应 contributor。
  - Mock：mock contributors。
- `registry_is_immutable_after_build`
  - 准备：build registry。
  - 执行：读取 contributor slices。
  - 断言：调用方无法通过 public API 修改 registry。
  - Mock：不需要。
- `turn_hooks_are_async_object_safe`
  - 准备：mock async hook。
  - 执行：作为 `Arc<dyn TurnHook>` 调用。
  - 断言：hook 被调用。
  - Mock：mock hook。

## Integration Tests

- `extension_tool_provider_registers_tools`
  - 准备：extension 提供 mock tool。
  - 执行：engine/app startup 注册 extensions。
  - 断言：ToolRouter 包含该工具。
  - Mock：mock extension。
- `extension_prompt_contributor_affects_prompt`
  - 准备：prompt contributor 返回 prompt section。
  - 执行：构建 provider request。
  - 断言：system prompt 包含 section。
  - Mock：mock provider。
- `extension_turn_hooks_run_in_order`
  - 准备：两个 hooks 记录顺序。
  - 执行：run_turn。
  - 断言：start/end hooks 按注册顺序调用。
  - Mock：mock provider/hooks。

## CLI/TUI Acceptance Tests

- `extension_ui_contributor_visible`
  - 准备：UI contributor 注册一个 status segment。
  - 执行：渲染 TUI status。
  - 断言：segment 可见。
  - Mock：TUI test backend。

## Failure Cases

- `extension_hook_error_does_not_panic`
  - 准备：hook 返回错误。
  - 执行：run_turn。
  - 断言：错误按策略记录或中止 turn，不 panic。
  - Mock：mock hook/provider。
- `duplicate_extension_contribution`
  - 准备：两个 extension 注册同名 tool。
  - 执行：registry/tool registration。
  - 断言：返回清晰冲突错误。
  - Mock：mock extensions。
