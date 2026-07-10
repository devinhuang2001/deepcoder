# Context Management Spec

## Current State

`deepcoder-engine` includes a context budget manager that estimates request size, filters reasoning-only content from provider history, preserves recent messages and tool metadata, errors on single over-budget messages, and tracks per-turn token usage. Resume/fork rebuild message history from persistence. Long-running semantic summaries remain a future quality improvement beyond the tested compaction strategy.

## ADDED Requirements

### Requirement: Context manager SHALL track token budget
The system MUST estimate or read token usage and keep requests within configured context limits.

#### Scenario: Budget calculation
- **WHEN** a provider request is built
- **THEN** system prompt, tools, skills, and history are counted or estimated
- **THEN** total budget is compared with provider context window

### Requirement: Context compaction SHALL preserve useful state
When history exceeds budget, the system MUST compact old conversation into summaries.

#### Scenario: Over budget
- **WHEN** context would exceed the configured limit
- **THEN** oldest non-critical history is summarized
- **THEN** recent user, assistant, and tool messages remain verbatim

#### Scenario: Tool result preservation
- **WHEN** compacting history with tool results
- **THEN** file paths, command outputs, errors, and edit summaries remain available

### Requirement: Context SHALL support resume and fork
Restored sessions MUST rebuild provider context consistently.

#### Scenario: Resume rebuild
- **WHEN** a session is resumed
- **THEN** compacted summaries and recent messages produce the same provider-visible history order

## Out of Scope for this phase

- Exact tokenizer parity if provider usage is unavailable.
- Vector retrieval or semantic memory.
