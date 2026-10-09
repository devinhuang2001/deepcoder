# DeepCoder 实现参考来源

本文档记录 DeepCoder 后续实现可使用的上游参考与限制，避免实现时混用许可证不兼容代码。

## 可直接适配：Codex

- 来源文件：`D:\AAAA学习\笔记\agent学习\codex-main.zip`
- 许可证：Apache-2.0
- 使用策略：可以直接迁移、改写或裁剪 Rust 实现，但需要保留许可证归属，并在发布材料中说明使用了 Apache-2.0 来源。
- 优先参考路径：
  - `codex-main/codex-rs/core/src/session/turn.rs`：agent turn loop、事件流、工具调用闭环。
  - `codex-main/codex-rs/core/src/exec.rs` 与 `codex-main/codex-rs/shell-command/`：shell 执行、输出处理、超时。
  - `codex-main/codex-rs/core/src/exec_policy.rs` 与 `codex-main/codex-rs/execpolicy/`：命令审批与策略。
  - `codex-main/codex-rs/apply-patch/`：补丁解析与文件编辑。
  - `codex-main/codex-rs/file-search/`：文件搜索、模糊搜索、grep/glob 参考。
  - `codex-main/codex-rs/app-server/` 与 `codex-main/codex-rs/app-server-protocol/`：JSON-RPC、线程/回合 API、WebSocket/in-process 传输。
  - `codex-main/codex-rs/codex-mcp/` 与 `codex-main/codex-rs/mcp-server/`：MCP client/server。
  - `codex-main/codex-rs/skills/`、`core-skills/`、`core/src/skills.rs`：技能发现、注入和筛选。
  - `codex-main/codex-rs/tui/`：ratatui 交互、diff/markdown/审批 UI。
  - `codex-main/codex-rs/state/`、`thread-store/`、`core/src/state/`：状态持久化、线程恢复、回放。

## 不能复制：Claude Code zip

- 来源文件：`D:\AAAA学习\笔记\agent学习\claude-code-main.zip`
- 许可证状态：包内 `LICENSE` 标明 `UNLICENSED — NOT FOR REDISTRIBUTION`，且声明为泄露的专有源码。
- 使用策略：不能复制、改写、派生或翻译该 zip 中的源码、文档、prompt、测试或架构细节。
- 允许参考范围：只参考公开官方文档、公开产品行为和用户明确描述的体验需求；不得使用该 zip 内的私有实现作为依据。

## 实施优先级

1. P1 工具系统和 tool loop 优先从 Codex Rust 实现裁剪适配。
2. AppServer、MCP、TUI、持久化优先复用 Codex 的协议与测试思路。
3. DeepSeek Provider 保留 DeepCoder 自有实现，只参考 Codex 的流式事件架构和错误处理模式。
4. 每次迁移上游代码时同步补充测试，并在相关文件头或发布文档中保留 Apache-2.0 归属说明。
