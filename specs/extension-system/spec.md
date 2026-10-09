# Extension System Spec

## Current State

`deepcoder-extension` has a builder and immutable registry for six contribution categories: tool providers, prompt contributors, turn hooks, event listeners, UI contributors, and persistence contributors. `ExtensionData` supports session/thread/turn scoped storage and async turn hooks are object-safe. Engine runtime integration points are defined but not yet used by default turns.

## ADDED Requirements

### Requirement: ExtensionRegistry SHALL be immutable after build
Extensions MUST be registered through a builder and exposed through read-only registry access.

#### Scenario: Registry construction
- **WHEN** builder registers contributors
- **THEN** `build()` returns an immutable registry
- **THEN** callers can read contributors but not mutate lists

### Requirement: Extension system SHALL define six contribution points
The registry MUST support tool providers, prompt contributors, turn hooks, event listeners, UI contributors, and persistence contributors.

#### Scenario: Tool provider contribution
- **WHEN** extension supplies tools
- **THEN** tools are registered in `ToolRouter`

#### Scenario: Prompt contributor
- **WHEN** prompt is built
- **THEN** prompt contributors can append deterministic prompt sections

#### Scenario: Turn hook
- **WHEN** turn starts or ends
- **THEN** async hooks are invoked in registration order

### Requirement: ExtensionData SHALL support scopes
Extensions MUST store data at session, thread, and turn scopes.

#### Scenario: Turn-scoped data
- **WHEN** a turn completes
- **THEN** turn-scoped extension data is dropped or persisted according to contributor policy

## Out of Scope for this phase

- Dynamic binary/plugin loading.
- Untrusted extension sandboxing.
