# Reasoning Stream Spec

## Current State

Reasoning deltas are parsed from DeepSeek SSE, emitted as `EngineEvent::ReasoningDelta`, displayed in the TUI reasoning panel, persisted as turn events/messages, counted in token estimates, and excluded from provider history when rebuilding chat requests. Full interactive show/hide controls beyond the existing config flag remain a UI polish item.

## ADDED Requirements

### Requirement: Reasoning tokens SHALL be parsed and accumulated
The system MUST treat `reasoning_content` separately from assistant answer text.

#### Scenario: Provider reasoning delta
- **WHEN** SSE includes `reasoning_content`
- **THEN** provider emits `ReasoningDelta`
- **THEN** engine accumulates it for the current turn

#### Scenario: Assistant message storage
- **WHEN** a turn includes reasoning
- **THEN** reasoning is stored as `ContentType::Reasoning`
- **THEN** it is not sent back as normal answer text in future requests

### Requirement: Reasoning display SHALL be optional
Users MUST be able to show or hide reasoning in UI surfaces that support it.

#### Scenario: TUI visible
- **WHEN** reasoning display is enabled
- **THEN** deltas appear in the reasoning panel

#### Scenario: TUI hidden
- **WHEN** reasoning display is disabled
- **THEN** answer text still streams normally
- **THEN** reasoning is retained for persistence if configured

### Requirement: Reasoning SHALL be persistable
Reasoning chunks MUST be written to JSONL when persistence is enabled.

#### Scenario: Persistence event
- **WHEN** reasoning delta arrives
- **THEN** a reasoning event with thread id, turn id, and content chunk is appended

## Out of Scope for this phase

- Displaying private chain-of-thought to external integrations by default.
- Reasoning summarization separate from context compaction.
