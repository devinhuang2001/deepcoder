## Why

DeepSeek V4 是一个强大的开源模型，具备优秀的工具调用（Function Calling）和长上下文能力。然而目前没有一个 Agent Harness CLI 是专门为 DeepSeek V4 深度优化的——Claude Code（Anthropic）是闭源绑定 Anthropic API 的，Codex CLI（OpenAI）是绑定 OpenAI Responses API 的。两者都用了大量工程精力适配各自的私有 API 特性（提示缓存、扩展思考等），而这些对 DeepSeek V4 用户毫无意义。

**DeepCoder** 的目标是：基于 Claude Code 和 Codex CLI 的最佳架构实践，用 Rust 构建一个**专门为 DeepSeek V4 优化的开源 Agent Harness**，充分利用 DeepSeek V4 的特性（长上下文、强推理、function calling），同时避免对其不利的模式（过度依赖系统提示压缩、专有 API 特性）。

## What Changes

构建 **DeepCoder** — 专为 DeepSeek V4 优化的 AI 编码助手 CLI：

- **Rust 原生二进制**：高性能、低延迟启动、跨平台静态编译
- **DeepSeek V4 优先适配**：API 层针对性优化（工具格式、上下文管理、推理配置）
- **深层推理集成**：利用 DeepSeek V4 的强推理能力，设计延展思考（Chain-of-Thought）解析和推理展示
- **分层架构**：CLI/AppServer/CoreEngine/Provider 四层解耦（借鉴 Codex）
- **类型化扩展系统**：6 种生命周期贡献点的 ExtensionRegistry
- **多安全沙箱**：进程加固 + 平台沙箱 + 执行策略（借鉴 Codex）
- **文件+内存持久化**：保持轻量，不需要 SQLite 的重量级依赖
- **MCP 双向支持**：既是 MCP Client 也是 MCP Server
- **丰富的终端 UI**：ratatui 驱动的聊天界面（ChatWidget、Markdown、Diff）
- **技能系统**：SKILL.md 加载 + 条件技能

## Capabilities

### New Capabilities

- `core-engine`: 核心 Agent 引擎 — 回合管理、工具路由、流式模型调用
- `tool-system`: 工具系统 — Tool trait、ToolRouter、权限检查、并行执行
- `deepseek-v4-provider`: DeepSeek V4 API 适配 — 工具格式、上下文管理、推理集成
- `cli-entry`: CLI 入口 — clap 多模式（交互/执行/MCP/AppServer）
- `tui-interface`: ratatui 终端 UI — 聊天、Markdown、Diff、流式渲染
- `app-server`: JSON-RPC 应用服务器 — Stdio/UDS/WebSocket 传输
- `extension-system`: 类型化扩展系统 — 6 种贡献点的 ExtensionRegistry
- `sandbox-security`: 安全沙箱 — 进程加固 + 平台沙箱 + 策略引擎
- `mcp-integration`: MCP 双向集成 — Client + Server
- `config-system`: TOML 分层配置 + 来源追踪
- `state-persistence`: 文件 + JSONL 轻量持久化
- `skill-system`: SKILL.md 技能系统
- `reasoning-stream`: 推理流解析 — 解析 DeepSeek V4 的推理 token 并 UI 展示
- `context-management`: 上下文管理 — 专为 DeepSeek V4 长上下文优化的策略

### Modified Capabilities

（无 — 全新项目）

## Impact

- **新项目**：Rust Cargo workspace，初始 40-50 crate
- **实现路径**：`D:\AAAA学习\笔记\agent学习\deepcoder`
- **构建系统**：Cargo workspace
- **平台支持**：macOS / Linux / Windows
- **核心依赖**：tokio、clap、ratatui、serde、reqwest、rmcp
