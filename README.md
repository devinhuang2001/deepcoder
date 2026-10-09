# DeepCoder 🧠

**专为 DeepSeek V4 优化的 AI 编码助手**

DeepCoder 是一个高性能 AI 编码助手，基于 Rust 构建，专门针对 DeepSeek V4 模型深度优化。它现在同时提供小白可双击启动的 Windows 桌面版、CLI、TUI、AppServer 和 MCP 模式。

## 架构

```
deepcoder/
├── Cargo.toml                    # Workspace 根
├── proposal.md                   # 提案文档
├── design.md                     # 架构设计
├── tasks.md                      # 实施任务清单
├── specs/                        # 系统规格说明
│   ├── core-engine/              #   核心引擎
│   ├── tool-system/              #   工具系统
│   ├── deepseek-v4-provider/     #   DeepSeek V4 适配
│   ├── cli-entry/                #   CLI 入口
│   ├── tui-interface/            #   终端 UI
│   ├── web-interface/            #   Web 工作台
│   ├── app-server/               #   应用服务器
│   ├── extension-system/         #   扩展系统
│   ├── sandbox-security/         #   安全沙箱
│   ├── mcp-integration/          #   MCP 集成
│   ├── config-system/            #   配置系统
│   ├── state-persistence/        #   状态持久化
│   ├── skill-system/             #   技能系统
│   ├── reasoning-stream/         #   推理流
│   ├── context-management/       #   上下文管理
│   ├── desktop-interface/        #   Windows 桌面版
│   └── integration-release/      #   集成测试与发布
└── deepcoder/                    # Rust 源码 (WORKSPACE)
    ├── Cargo.toml
    ├── deepcoder-cli/            #   CLI 入口
    ├── deepcoder-desktop/        #   Windows 桌面版
    ├── deepcoder-tui/            #   TUI 界面
    ├── deepcoder-engine/         #   核心引擎
    ├── deepcoder-provider/       #   Provider
    ├── deepcoder-tools/          #   工具系统
    ├── deepcoder-config/         #   配置系统
    ├── deepcoder-sandbox/        #   沙箱
    ├── deepcoder-app-server/     #   AppServer
    ├── deepcoder-extension/      #   扩展系统
    ├── deepcoder-mcp/            #   MCP 集成
    ├── deepcoder-persistence/    #   持久化
    ├── deepcoder-skills/         #   技能系统
    ├── deepcoder-types/          #   共享类型
    └── deepcoder-error/          #   错误类型
```

## 快速开始

### 小白启动

Windows 用户优先双击项目根目录的：

```text
启动DeepCoder.bat
```

启动脚本会自动设置推荐的 D 盘构建/临时目录，并按顺序查找：

1. 根目录 `DeepCoder桌面版.exe`
2. `D:\deepcoder-target\release\deepcoder-desktop.exe`
3. 本机 `cargo run -p deepcoder-desktop --release`

首次打开桌面版时，如果没有检测到 API Key，会显示配置向导。填写 API Key、模型和 Base URL 后，会保存到现有 DeepCoder 全局配置文件，保存后无需重启。

如果想使用浏览器里的 Codex 风格 Web 工作台，双击：

```text
启动DeepCoder网页版.bat
```

网页版启动脚本会自动设置 `CARGO_TARGET_DIR=D:\deepcoder-target`、`TMP/TEMP=D:\codex-temp`，启动 `deepcoder app-server --ws 127.0.0.1:8080`，启动 `deepcoder-web` 的 Vite dev server，并打开 `http://127.0.0.1:5173`。日志写入根目录 `logs/deepcoder-app-server.log` 和 `logs/deepcoder-web.log`。如需改端口，可设置 `DEEPCODER_WS_ADDR` 或 `DEEPCODER_WEB_PORT` 环境变量。

构建桌面版并复制到根目录：

```powershell
powershell -ExecutionPolicy Bypass -File .\构建桌面版.ps1
```

开发环境下直接启动桌面版：

```powershell
cd .\deepcoder
$env:CARGO_TARGET_DIR='D:\deepcoder-target'
$env:TEMP='D:\codex-temp'
$env:TMP='D:\codex-temp'
cargo run -p deepcoder-desktop
```

### 命令行启动

```bash
# 构建
cargo build --release

# 交互模式（默认）
./deepcoder

# 批处理模式
./deepcoder exec "解释一下这个项目"

# MCP 服务器模式
./deepcoder mcp-server

# AppServer 模式（IDE 集成）
./deepcoder app-server --ws 127.0.0.1:8080

# AppServer stdio JSON-RPC
./deepcoder app-server --stdio
```

## 当前实现状态

当前仓库已具备可编译的 Rust workspace、Windows 桌面版、CLI/TUI/Engine/Provider 框架、DeepSeek SSE 解析与重试、分片 tool_call 参数累积、内置文件/搜索/Shell/Web/task/agent 工具、workspace 内路径边界、tool loop、多工具并发安全策略、桌面/TUI 审批通路、JSONL 持久化、session resume/fork、桌面/TUI SessionPicker、token usage 估算、技能注入、上下文压缩、AppServer JSON-RPC/thread/turn/cancel API、WebSocket `turn/event` 实时通知和运行中 turn 抢占取消、Web 真实 thread/list 会话侧边栏与历史恢复、Web reasoning/tool/token 右侧面板、Web 三栏视觉重构与移动端紧凑顶栏、桌面版和网页版一键启动、stdio/WebSocket/in-process AppServer transport、MCP client/server `initialize`/`tools/list`/`tools/call`、配置 `get/set/list`、扩展注册表六类贡献点、CI、release workflow，以及带强令牌鉴权、精确 Origin allowlist、限流、健康检查、持久卷和非 root 运行时的公网单容器。平台 sandbox 当前是策略门禁与危险命令 fail-closed，不声称完整 OS 级隔离；`AgentTool` 当前提供隔离 agent 任务契约，不在 tools crate 中递归调用真实 provider。

实现参考来源与许可证策略见 `REFERENCE_SOURCES.md`。Codex 的 Apache-2.0 Rust 实现可直接适配；未授权或专有来源不得复制进本项目。

推荐验证命令（Windows 中文路径或 C 盘临时目录空间不足时使用 D 盘 target/temp）：

```powershell
New-Item -ItemType Directory -Force -Path D:\deepcoder-target,D:\codex-temp | Out-Null
$env:CARGO_TARGET_DIR='D:\deepcoder-target'
$env:TEMP='D:\codex-temp'
$env:TMP='D:\codex-temp'
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo test --workspace
cargo build --release -p deepcoder-desktop
cd ..\deepcoder-web
npm test
npm run build
```

## 配置

```toml
# ~/.config/deepcoder/config.toml
[provider]
type = "deepseek"
model = "deepseek-chat"  # 默认
base_url = "https://api.deepseek.com"

[sandbox]
mode = "auto"            # auto / enforce / off

[ui]
theme = "default"

[system]
max_tool_iterations = 25
max_context_tokens = 131072
```

运行时 API Key 当前推荐通过环境变量或 CLI 参数设置：

```bash
export DEEPSEEK_API_KEY=sk-your-key
deepcoder --api-key sk-your-key exec "hello"
```

容器部署还支持以下环境变量：

- `DEEPCODER_ACCESS_TOKEN`：32–256 位 URL 安全访问令牌。
- `DEEPCODER_ALLOWED_ORIGINS`：逗号分隔的精确 `https://host[:port]` Origin。
- `DEEPCODER_DATA_DIR`：持久化 session/index 的目录。
- `DEEPCODER_WORKSPACE_DIR`：容器内工具可操作的持久 workspace。

常用配置命令：

```bash
deepcoder config list
deepcoder config get provider.model
deepcoder config set provider.model deepseek-chat
```

## 发布

CI 已配置三平台 `fmt`、`clippy`、`test`、`build`、Web 单测/构建和生产 Docker 镜像构建。推送 `v*` tag 会触发 release workflow，生成包含目标平台 triple 的压缩包和 `.sha256` 校验文件；Windows 包会附带 `DeepCoder桌面版.exe`、桌面/网页版一键启动脚本、Web 源文件与预构建 `dist`。下载后将 `deepcoder`/`deepcoder.exe` 放入 `PATH`，再设置 `DEEPSEEK_API_KEY` 或运行 `deepcoder --api-key ... exec "hello"` 验证。

### 公网 Web 部署（Render）

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/devinhuang2001/deepcoder)

根目录 `render.yaml` 会创建 Docker Web Service、`/var/data` 持久磁盘和 `/health` 探针。首次部署时填写 `DEEPSEEK_API_KEY` 和 `DEEPCODER_ACCESS_TOKEN`。可用 PowerShell 生成访问令牌：

```powershell
[guid]::NewGuid().ToString('N') + [guid]::NewGuid().ToString('N')
```

默认生产 Origin 是 `https://deepcoder-devinhuang2001.onrender.com`；如果重命名 Render 服务，必须同步修改 `DEEPCODER_ALLOWED_ORIGINS`。部署成功后访问站点、输入同一个访问令牌，再用 `/health` 验证服务。令牌只保存在页面内存中，不进入 `localStorage`、构建产物或仓库。

这是单用户安全部署，不是多租户 SaaS。AppServer 含文件与 Shell 工具，请勿共享访问令牌，也不要绕过 HTTPS/WSS 或直接公开内部 `8081` 端口。Render 持久磁盘要求付费实例；没有持久盘时会话和 workspace 会在实例替换后丢失。

## 特性

- 🚀 **Rust 原生** — 低延迟、低内存、高性能
- 🧠 **DeepSeek V4 深度优化** — 推理 token 展示、工具调用优化、长上下文策略
- 🏗️ **分层架构** — CLI/TUI/Engine/Provider 四层分离
- 🔧 **丰富工具** — 文件、Shell、搜索、Web、子 Agent、MCP
- 🔒 **安全边界** — workspace 路径限制 + Shell 策略门禁 + 公网强令牌/Origin/限流 + 危险命令 fail-closed
- 🔌 **MCP 双向** — 既是 Client 也是 Server
- 🖥️ **Windows 桌面版** — 聊天工作台、配置向导、会话列表、工具审批
- 🌐 **Web 工作台** — 一键启动 AppServer + Web，三栏工程工作台、移动端紧凑布局、真实会话/历史、reasoning/tool/token 面板和运行中 turn 停止
- 🎨 **ratatui TUI** — 推理面板、Markdown、Diff、流式渲染

## 许可证

[MIT](LICENSE)
