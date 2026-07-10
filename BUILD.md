# DeepCoder 构建指南

## 环境要求

- **Rust 1.85+**（Edition 2024 最低要求，推荐 1.96+）
- **Cargo** (随 Rust 安装)
- **Node.js 22+ / npm 10+**（构建 Web）
- **Docker**（构建公网单容器，可选）

## 构建步骤

### 1. 安装 Rust

```bash
# 访问 https://rustup.rs 下载安装
# Windows 推荐使用 x86_64-pc-windows-msvc 工具链
# 安装后需重启终端
```

### 2. 配置环境变量

```bash
# 设置 DeepSeek API Key（运行时需要）
export DEEPSEEK_API_KEY=sk-your-key-here
```

### 3. 构建

```bash
# 进入项目目录
cd D:\AAAA学习\笔记\agent学习\deepcoder\deepcoder

# 构建全部
cargo build --release

# 或仅构建核心库
cargo build -p deepcoder-types -p deepcoder-error -p deepcoder-config -p deepcoder-tools -p deepcoder-provider -p deepcoder-engine

# 完整构建（含 CLI）
cargo build --release -p deepcoder-cli
```

### 4. 运行

```bash
# 交互模式（默认）
./target/release/deepcoder

# 配置 API Key
export DEEPSEEK_API_KEY=sk-xxx
deepcoder

# 批处理模式
deepcoder exec "用中文解释 Rust 的所有权系统"

# 使用配置文件
# 创建 ~/.config/deepcoder/config.toml:
# [provider]
# model = "deepseek-chat"
# base_url = "https://api.deepseek.com"
```

### 5. 构建生产容器

根目录 `Dockerfile` 会构建 React 静态资源、Rust CLI/AppServer，并用 Caddy 在同一个端口提供 `/`、`/ws` 和 `/health`：

```powershell
docker build -t deepcoder:local .
$token = [guid]::NewGuid().ToString('N') + [guid]::NewGuid().ToString('N')
docker run --rm -p 10000:10000 `
  -e DEEPCODER_ACCESS_TOKEN=$token `
  -e DEEPCODER_ALLOWED_ORIGINS=http://127.0.0.1:10000 `
  -e DEEPSEEK_API_KEY=sk-your-key `
  -v deepcoder-data:/var/data `
  deepcoder:local
```

打开 `http://127.0.0.1:10000`，输入 `$token`。公网环境必须使用 HTTPS/WSS；生产部署还应保留持久卷 `/var/data`。

## 常见问题

### Windows 链接错误: "link: extra operand"

**原因**: 项目路径包含中文字符，或 `link.exe` 指向了 Git Bash 的链接命令而非 MSVC 链接器。

**解决方案**:
```bash
# 方案 A: 复制到纯 ASCII 路径编译后移回
cp -r deepcoder /tmp/deepcoder-build
cd /tmp/deepcoder-build
cargo build --release
cp target/release/deepcoder.exe ../deepcoder/

# 方案 B: 使用 GNU 工具链
rustup default stable-x86_64-pc-windows-gnu
# 确保 Git Bash 的 link.exe 不在 PATH 中

# 方案 C: 安装 Visual Studio Build Tools
# 下载安装 https://visualstudio.microsoft.com/visual-cpp-build-tools/
# 选择 "C++ 生成工具" 工作负载
```

### 找不到 DEEPSEEK_API_KEY

当前推荐使用环境变量或 CLI 参数设置:
```bash
set DEEPSEEK_API_KEY=sk-your-key
deepcoder --api-key sk-your-key exec "hello"
```

### Windows 临时目录或 C 盘空间不足

如果路径包含中文，或 MinGW 在 `C:\Users\...\Temp` 创建临时文件失败，可把构建产物和临时目录切到 D 盘：

```powershell
New-Item -ItemType Directory -Force -Path D:\deepcoder-target,D:\codex-temp | Out-Null
$env:CARGO_TARGET_DIR='D:\deepcoder-target'
$env:TEMP='D:\codex-temp'
$env:TMP='D:\codex-temp'
cargo check --workspace
cargo test --workspace
```

## 项目结构

```
deepcoder/               # Cargo workspace
├── Cargo.toml           # workspace 根
├── deepcoder-cli/       # CLI 入口
├── deepcoder-tui/       # TUI 界面 (ratatui)
├── deepcoder-engine/    # 核心引擎
├── deepcoder-provider/  # DeepSeek V4 Provider
├── deepcoder-tools/     # 工具系统
├── deepcoder-config/    # 配置系统
├── deepcoder-sandbox/   # 安全沙箱
├── deepcoder-app-server/# JSON-RPC 服务器
├── deepcoder-extension/ # 扩展系统
├── deepcoder-mcp/       # MCP 集成
├── deepcoder-persistence/# 持久化
├── deepcoder-skills/    # 技能系统
├── deepcoder-types/     # 共享类型
└── deepcoder-error/     # 错误类型
```
