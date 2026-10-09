# Web Interface Spec

## Current State

`deepcoder-web` 是浏览器版 Codex 风格工作台，使用 React/Vite/Tailwind，通过 AppServer WebSocket 连接 Rust 引擎。当前 Web 工作台已实现 `initialize`、`thread/list`、`thread/create`、`thread/get`、`turn/start`、`turn/cancel` 和 `turn/event` 事件处理；左侧显示真实会话列表，中央显示用户/助手正文，右侧显示 reasoning、工具事件和 token usage。生产构建使用同源 `/ws`、运行时内存令牌登录和指数退避重连；令牌不写入浏览器存储或 Vite 构建变量。根目录提供一键启动脚本，也提供 Docker/Caddy 公网单容器。

## ADDED Requirements

### Requirement: Web interface SHALL connect to AppServer WebSocket
The Web interface MUST use JSON-RPC over WebSocket instead of simulated responses.

#### Scenario: Initial connection
- **WHEN** the browser loads the app
- **THEN** it connects to an explicit `VITE_DEEPCODER_WS_URL` or same-origin `/ws`
- **THEN** it offers the `deepcoder-v1` WebSocket subprotocol
- **THEN** it calls `initialize`
- **THEN** it loads existing sessions with `thread/list`

#### Scenario: Connection failure
- **WHEN** WebSocket connection fails
- **THEN** the UI displays an actionable disconnected state
- **THEN** pending JSON-RPC promises are rejected

### Requirement: Web interface SHALL manage real sessions
The Web interface MUST use AppServer thread APIs for session state.

#### Scenario: Existing sessions
- **WHEN** `thread/list` returns sessions
- **THEN** the sidebar renders those sessions
- **THEN** selecting a session calls `thread/get`
- **THEN** persisted messages are rendered in the chat transcript

#### Scenario: New session
- **WHEN** the user clicks new session
- **THEN** the app calls `thread/create`
- **THEN** the new thread becomes active

### Requirement: Web interface SHALL stream turn events
The Web interface MUST render live model output from AppServer notifications.

#### Scenario: Text streaming
- **WHEN** AppServer sends `turn/event` with `type=text_delta`
- **THEN** the active assistant message is appended incrementally

#### Scenario: Reasoning and tool details
- **WHEN** AppServer sends reasoning or tool events
- **THEN** the right panel updates without duplicating those details into the final chat bubble

#### Scenario: Token usage
- **WHEN** a turn completes
- **THEN** token usage from `turn_complete` or final response is displayed in the right panel

### Requirement: Web interface SHALL cancel running turns
The Web interface MUST provide a stop button while a turn is running.

#### Scenario: Stop running turn
- **WHEN** a turn is active
- **THEN** the send button changes to a stop button
- **WHEN** the user clicks stop
- **THEN** the app calls `turn/cancel` with the current client `turn_id`
- **THEN** the UI displays the cancelled state and clears busy state

### Requirement: Web interface SHALL provide a polished responsive workbench
The Web interface MUST present the coding assistant as a usable workbench on desktop and mobile widths.

#### Scenario: Desktop workbench
- **WHEN** the viewport is at least desktop width
- **THEN** the UI shows sessions, chat, and diagnostics as separate scan-friendly regions
- **THEN** message bubbles, input controls, and status badges do not overlap or overflow

#### Scenario: Mobile workbench
- **WHEN** the viewport is narrow
- **THEN** the UI keeps the chat and input usable in a single-column layout
- **THEN** the top bar exposes a compact new-session action and current runtime status

### Requirement: Web launcher SHALL support one-click Windows startup
Windows users MUST be able to start the Web workbench without manually running AppServer and Vite.

#### Scenario: Double-click launch
- **WHEN** the user runs `启动DeepCoder网页版.bat`
- **THEN** the launcher prepares D drive target/temp directories
- **THEN** it starts AppServer if port `8080` is not already listening
- **THEN** it starts Vite if port `5173` is not already listening
- **THEN** it opens `http://127.0.0.1:5173`

#### Scenario: Validation-only launch
- **WHEN** CI or a maintainer runs `StartDeepCoderWeb.ps1 -ValidateOnly`
- **THEN** the script checks required directories and files
- **THEN** it exits without starting background services or opening a browser

### Requirement: Production Web SHALL authenticate without persisting secrets

#### Scenario: Secure login
- **WHEN** the production page loads
- **THEN** it requests the deployment access token before opening AppServer
- **THEN** it sends the token only in the WebSocket subprotocol handshake
- **THEN** it keeps the token only in page memory for reconnects

#### Scenario: Connection replacement
- **WHEN** an authenticated WebSocket disconnects unexpectedly
- **THEN** the client reconnects with capped exponential backoff
- **THEN** a failed initial authentication returns to the token form

## Out of Scope for this phase

- Browser-side API key configuration wizard; desktop app remains the first-run guided path.
- Multi-user accounts, RBAC, billing, or tenant-isolated workspaces.
