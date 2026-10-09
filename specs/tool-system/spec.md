# Tool System Spec

## Current State

`deepcoder-tools` 已实现 `Tool` trait、`ToolRouter`、审批 approver 通路、direct specs、并发安全声明和内置工具注册。内置工具覆盖 `read_file`、`write_file`、`edit_file`、`bash`、`glob`、`grep`、`web_fetch`、`web_search`、`agent` 与 `task_create/list/update/delete`。文件和搜索工具使用 `ToolContext.workspace_root` 限制路径边界，拒绝 workspace 外访问。`bash` 支持 `ExecPolicy`、prompt 审批、timeout、stdout/stderr/exit code/duration 和配置化 sandbox 检查。

## ADDED Requirements

### Requirement: Tool trait SHALL define the public tool contract
Every tool MUST provide a stable name, JSON schema, visibility, permission behavior, and async execution result.

#### Scenario: Tool definition
- **WHEN** a tool is registered
- **THEN** it MUST provide a unique `name`
- **THEN** it MUST provide a `ToolSpec` with JSON input schema
- **THEN** it MUST return `JsonToolOutput` from `call`

#### Scenario: Tool exposure
- **WHEN** a tool uses `ToolExposure::Direct`
- **THEN** it appears in the initial model tool list
- **WHEN** a tool uses `ToolExposure::Deferred`
- **THEN** it is hidden from initial tools but searchable later
- **WHEN** a tool uses `ToolExposure::Hidden`
- **THEN** it can be dispatched internally but is never exposed to the model

### Requirement: ToolRouter SHALL dispatch tools by name
The router MUST execute registered tools and return clear errors for unknown or denied calls.

#### Scenario: Known tool
- **WHEN** a `ToolCall` references a registered tool name
- **THEN** Router checks permissions
- **THEN** Router calls the tool with arguments and context
- **THEN** Router returns the tool output

#### Scenario: Unknown tool
- **WHEN** a `ToolCall` references an unregistered tool name
- **THEN** Router returns `DeepCoderError::ToolNotFound`

#### Scenario: Permission denied
- **WHEN** `check_permissions` returns deny
- **THEN** Router returns `DeepCoderError::ToolDenied`
- **THEN** the tool MUST NOT execute

### Requirement: Built-in coding tools SHALL be available
DeepCoder MUST ship file, shell, search, web, and task/agent tools.

#### Scenario: File tools
- **WHEN** `read_file`, `write_file`, or `edit_file` is called
- **THEN** the tool validates paths
- **THEN** absolute paths and parent traversal MUST stay inside `workspace_root`
- **THEN** it returns structured success or error data
- **THEN** edits include enough diff information for TUI/AppServer display

#### Scenario: Shell tool
- **WHEN** `bash` is called
- **THEN** it evaluates `ExecPolicy`
- **THEN** it runs with timeout and sandbox settings
- **THEN** it returns stdout, stderr, exit code, and duration

#### Scenario: Search tools
- **WHEN** `glob` or `grep` is called
- **THEN** it returns matching paths or lines with file and line metadata
- **THEN** search roots and glob static prefixes MUST stay inside `workspace_root`
- **THEN** invalid patterns return structured tool errors

#### Scenario: Web tools
- **WHEN** `web_fetch` or `web_search` is called
- **THEN** network failures, non-2xx responses, and timeouts are reported as tool errors

### Requirement: Concurrency policy SHALL be explicit
Tools MUST declare whether they are safe to execute concurrently.

#### Scenario: Parallel safe tools
- **WHEN** multiple concurrency-safe tools are requested in one model response
- **THEN** the engine MAY execute them in parallel

#### Scenario: Unsafe tools
- **WHEN** a tool is not concurrency-safe
- **THEN** the engine MUST execute it sequentially relative to other unsafe tools

## Out of Scope for this phase

- Full sub-agent orchestration beyond a minimal `AgentTool` contract.
- Browser automation tools.
- Real web search provider selection beyond a mockable provider interface.
