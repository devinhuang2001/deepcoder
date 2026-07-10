# Core Engine Spec

## Current State

`deepcoder-engine` can create, resume, and fork sessions; run streamed turns; emit text/reasoning/tool/complete/error events; execute tool loops with follow-up model requests; run concurrency-safe tools in parallel; persist turn lifecycle events and messages; inject active skills; estimate token usage; and apply context compaction before provider calls.

## ADDED Requirements

### Requirement: Agent loop SHALL manage turn lifecycle
The engine MUST process a user input through model streaming, tool execution, optional follow-up model calls, and turn completion.

#### Scenario: Basic query-response turn
- **WHEN** user submits a text query
- **THEN** the engine appends a user message
- **THEN** the engine sends visible history and tool specs to the provider
- **THEN** text and reasoning deltas are emitted as `EngineEvent`
- **THEN** the turn completes when the provider emits done without pending tool calls

#### Scenario: Tool-call loop
- **WHEN** the model requests one or more tool calls
- **THEN** the engine executes tools through `ToolRouter`
- **THEN** tool results are appended as tool messages
- **THEN** the engine sends another provider request with the updated history
- **THEN** the loop continues until the model response has no tool calls

#### Scenario: Maximum tool iterations
- **WHEN** tool iterations exceed `system.max_tool_iterations`
- **THEN** the engine returns `DeepCoderError::Turn`
- **THEN** the turn emits an error event

### Requirement: Prompt construction SHALL be deterministic
The engine MUST build provider messages from system prompt, skills, tools, and conversation history in a stable order.

#### Scenario: Reasoning history excluded
- **WHEN** previous messages contain reasoning content
- **THEN** reasoning content is not sent back as normal user/assistant text

#### Scenario: Tool specs included
- **WHEN** direct tools are registered
- **THEN** their specs are included in the provider request

### Requirement: Session state SHALL support resume and fork
The engine MUST expose stable session/thread state for persistence and UI selection.

#### Scenario: Resume session
- **WHEN** a saved session is loaded
- **THEN** messages, thread metadata, and token usage are restored

#### Scenario: Fork session
- **WHEN** a session is forked
- **THEN** a new session id and thread id are created
- **THEN** original history remains unchanged

### Requirement: Token usage SHALL be tracked
The engine MUST aggregate provider usage per turn and across the session.

#### Scenario: Turn completion
- **WHEN** a provider response includes usage
- **THEN** input, output, reasoning, and total tokens are recorded
- **THEN** the TUI/AppServer can display cumulative usage

## Out of Scope for this phase

- Model-specific tokenization accuracy beyond provider-reported usage.
- Multi-agent scheduling outside `AgentTool`.
- Distributed session storage.
