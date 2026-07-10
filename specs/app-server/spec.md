# AppServer Spec

## Current State

`deepcoder-app-server` 已实现 JSON-RPC 2.0 `initialize`、thread CRUD、`turn/start`、`turn/cancel` 协议方法，并接入持久化与真实 engine turn。`thread/get` 返回线程元数据和持久化 messages，供 Web/IDE 恢复历史对话。传输层支持 stdio JSON lines、WebSocket 和 in-process channel；WebSocket transport 会在 `turn/start` 执行期间推送 `turn/event` JSON-RPC notification，并在完成后返回最终 response。公网模式要求强访问令牌、精确 Origin allowlist、`deepcoder-v1` 子协议、有界队列和连接/消息/回合限制；无令牌绑定非 loopback 地址 MUST fail closed。测试使用 mock DeepSeek SSE，不依赖真实 API key。

## ADDED Requirements

### Requirement: AppServer SHALL provide JSON-RPC 2.0 API
The AppServer MUST parse requests, route methods, and return JSON-RPC compliant responses.

#### Scenario: Valid request
- **WHEN** a valid JSON-RPC request arrives
- **THEN** MessageProcessor routes by `method`
- **THEN** response contains matching `id`

#### Scenario: Unknown method
- **WHEN** method is not registered
- **THEN** response error code is `-32601`

#### Scenario: Invalid params
- **WHEN** params cannot be decoded
- **THEN** response error code is `-32602`

### Requirement: AppServer SHALL expose thread and turn APIs
IDE integrations MUST manage threads and run turns through RPC.

#### Scenario: Thread CRUD
- **WHEN** client calls thread create/list/get/delete
- **THEN** AppServer updates persistence and returns thread metadata

#### Scenario: Thread history restore
- **WHEN** a client calls `thread/get` for a persisted thread
- **THEN** AppServer returns `thread` metadata
- **THEN** AppServer returns persisted `messages` in chronological order

#### Scenario: Turn lifecycle
- **WHEN** client starts a turn
- **THEN** AppServer streams text, reasoning, tool, and completion events
- **THEN** WebSocket clients receive `turn/event` notifications before the final response

#### Scenario: Running turn cancellation
- **WHEN** a WebSocket client starts a turn with `turn_id`
- **AND** the client sends `turn/cancel` for the same `turn_id` before provider completion
- **THEN** AppServer aborts the running turn task
- **THEN** the original `turn/start` request receives a cancellation error
- **THEN** the cancelling request receives `cancelled=true`
- **THEN** clients receive a `turn/event` notification with `type=turn_cancelled`

### Requirement: AppServer SHALL support multiple transports
The same protocol MUST work over stdio, WebSocket, and in-process channels.

#### Scenario: WebSocket transport
- **WHEN** AppServer starts with a ws address
- **THEN** clients can connect and exchange JSON-RPC messages

#### Scenario: In-process transport
- **WHEN** TUI runs in the same process
- **THEN** it can communicate without TCP

### Requirement: AppServer SHALL fail closed when exposed remotely
The WebSocket transport MUST authenticate and constrain production clients before exposing coding tools.

#### Scenario: Authenticated production handshake
- **WHEN** a client offers `deepcoder-v1` and exactly one valid `deepcoder-token.<token>` protocol
- **AND** its Origin exactly matches `DEEPCODER_ALLOWED_ORIGINS`
- **THEN** the server upgrades the connection and echoes only `deepcoder-v1`

#### Scenario: Missing production security configuration
- **WHEN** production mode is enabled or a non-loopback listener is requested
- **AND** the access token or Origin allowlist is missing or invalid
- **THEN** AppServer exits before accepting requests

#### Scenario: Resource abuse
- **WHEN** a connection exceeds frame, message, rate, queue, connection, or running-turn limits
- **THEN** AppServer rejects or closes the connection without unbounded allocation

## Out of Scope for this phase

- Multi-user remote hosting.
