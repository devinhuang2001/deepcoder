## Context

基于 Anthropic Claude Code CLI 和 OpenAI Codex CLI 两个项目的深度源码分析，结合 DeepSeek V4 的模型特性，设计一个专门优化的 Agent Harness。

**DeepSeek V4 关键特性**：
- 长上下文窗口（128K+ tokens），降低对激进压缩的需求
- 优秀的推理能力（Chain-of-Thought），需要前端展示推理过程
- Function Calling API 接近 OpenAI 格式
- 开源可自部署，无专有 API 锁定的风险

**借鉴来源**：
- Claude Code：功能完整度（工具丰富、技能系统、MCP）、React/Ink TUI 设计理念
- Codex CLI：架构清晰度（分层、扩展系统、沙箱）、Rust 工程化、工具 trait 模式

## Goals / Non-Goals

**Goals：**
- DeepSeek V4 API 深度适配（工具调用、推理 token 处理、上下文策略）
- 分层架构：CLI/TUI ↔ AppServer ↔ CoreEngine ↔ Provider
- Tool trait 定义 + ToolRouter 路由分发
- 类型化 ExtensionRegistry（6 种贡献点）
- 安全沙箱：加固 + 平台隔离 + 策略
- 文件 + JSONL 轻量持久化
- MCP 双向支持（Client + Server 双模式）
- ratatui TUI（流式渲染、推理展示、Markdown、Diff）
- SKILL.md 技能系统

**Non-Goals：**
- 不提供语音/视频多模态支持
- 不内置专有 API 特性（提示缓存等绑定特定厂商的功能）
- 不提供桌面 GUI 应用

## Decisions

### D1: Rust + Cargo workspace

**选择**：Rust 2024 edition，tokio async runtime
**理由**：性能、安全、跨平台。对比 TypeScript 方案启动快 10-100x，内存低 5x

### D2: 四层分离 + AppServer 解耦

```
DeepCoder CLI (clap)
    ├── interactive → TUI (ratatui) ↔ InProcessAppServerClient (channel)
    ├── exec       → CoreEngine (直接调用)
    ├── mcp-server → MCPServer (rmcp)
    └── app-server → AppServer (JSON-RPC, WebSocket/UDS)
                         ↓
              CoreEngine (Session → run_turn)
                  ↓            ↓
         ToolRouter    ModelClient(DeepSeek V4)
```

### D3: Tool trait + ToolSpec 分离

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn spec(&self) -> ToolSpec;
    fn exposure(&self) -> ToolExposure { ToolExposure::Direct }
    async fn call(&self, params: Value) -> Result<ToolOutput, ToolError>;
    fn check_permissions(&self, input: &Value) -> PermissionResult { PermissionResult::Allow }
    fn is_concurrency_safe(&self) -> bool { false }
}
```

`ToolSpec` 与序列化格式分离，通过 `ModelProvider` 适配器转换成 DeepSeek V4 / OpenAI / Anthropic 各自的工具格式。

### D4: DeepSeek V4 优先适配

- 原生支持 DeepSeek V4 的 `reasoning_content` 推理 token 流式输出
- 推理过程在 TUI 中以独立区域展示（类比 Claude Code 的 thinking 显示）
- Function Calling 依 DeepSeek 官方格式最优适配
- 上下文管理策略针对 128K+ 长上下文优化（事实验证、摘要策略）

### D5: 轻量持久化（文件 + JSONL）

选文件持久化而非 SQLite，理由：
- 避免 SQLite 编译依赖和交叉编译复杂度
- DeepCoder 初始阶段不需要复杂查询
- JSONL 天然可审计、可 grep、可版本控制
- Session 文件按日期分片

### D6: ratatui TUI + 推理展示

- 主聊天区域显示对话
- 推理面板展示 DeepSeek V4 的 `reasoning_content`
- 状态行显示会话信息、token 使用
- Markdown 渲染使用 pulldown-cmark + syntect 语法高亮

### D7: 三层沙箱安全

1. 进程加固 (ctor)：禁用 core dump、ptrace、环境变量清理
2. 平台沙箱：Linux Landlock/bwrap、macOS Seatbelt、Windows RestrictedToken
3. 执行策略：PrefixRule 通配符匹配

## Risks / Trade-offs

- **[DeepSeek API 变动]** DeepSeek V4 API 可能持续更新 → Provider 层抽象隔离变动，适配器模式
- **[Rust 编译速度]** 40-50 crate workspace 编译慢 → cargo-watch 增量编译，CI 缓存
- **[Windows 沙箱]** Windows 沙箱能力有限 → RestrictedToken 最小可行方案，后续增强
- **[TUI 复杂度]** ratatui 的声明式能力不如 React/Ink → 函数组合模式 + 自定义 widget 抽象
