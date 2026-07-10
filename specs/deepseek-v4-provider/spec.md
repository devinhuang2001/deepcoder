# DeepSeek V4 Provider Spec

## Current State

`deepcoder-provider` has a `ModelProvider` trait and a DeepSeek implementation that POSTs to `/chat/completions`, converts internal tool specs to function-calling payloads, streams SSE lines, accumulates text/reasoning/tool-call deltas, handles `[DONE]`, tolerates EOF after the last valid delta, classifies non-2xx/connection/bad JSON errors, and retries retryable HTTP statuses with backoff. Tests use local mock HTTP/SSE.

## ADDED Requirements

### Requirement: Provider SHALL support DeepSeek Chat streaming
The provider MUST send Chat API requests and return an async stream of model events.

#### Scenario: Request body
- **WHEN** `ChatRequest` is sent
- **THEN** the body includes model, messages, stream flag, optional tools, max tokens, and temperature
- **THEN** the Authorization header uses the configured API key

#### Scenario: Text delta
- **WHEN** an SSE event contains `delta.content`
- **THEN** the provider emits `StreamEvent::TextDelta`

#### Scenario: Reasoning delta
- **WHEN** an SSE event contains `delta.reasoning_content`
- **THEN** the provider emits `StreamEvent::ReasoningDelta`

#### Scenario: Tool calls
- **WHEN** an SSE event contains `delta.tool_calls`
- **THEN** the provider emits a tool-call event with id, function name, and JSON arguments
- **THEN** partial argument chunks are accumulated before dispatch when DeepSeek sends fragmented tool calls

#### Scenario: Done event
- **WHEN** SSE data is `[DONE]`
- **THEN** the provider emits `StreamEvent::Done`
- **THEN** the stream ends after the done event

### Requirement: Provider SHALL classify failures
Network, HTTP, auth, rate-limit, timeout, and JSON parse failures MUST map to clear DeepCoder errors.

#### Scenario: Non-success status
- **WHEN** DeepSeek returns non-2xx
- **THEN** the provider returns an API/provider error containing status and response body snippet

#### Scenario: Invalid SSE JSON
- **WHEN** a data line is not valid JSON
- **THEN** the provider returns a serialization/provider error

### Requirement: Provider SHALL be testable without live API
All provider behavior MUST be covered through mock HTTP/SSE tests.

#### Scenario: Mock base URL
- **WHEN** config sets a test base URL
- **THEN** provider sends requests to that URL
- **THEN** tests do not require `DEEPSEEK_API_KEY`

## Out of Scope for this phase

- Multi-provider routing.
- OAuth or browser login flows.
- Vision or file upload endpoints.
