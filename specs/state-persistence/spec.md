# State Persistence Spec

## Current State

`deepcoder-persistence` appends JSONL events, reads event logs, maintains a TOML session index, records messages, lists/deletes sessions, reconstructs snapshots, and forks sessions with metadata. Engine turns record lifecycle events and messages. Automatic semantic summaries remain future enhancement; manual summary fields are supported in the index.

## ADDED Requirements

### Requirement: Sessions SHALL persist to JSONL
The system MUST record durable event logs for session recovery and debugging.

#### Scenario: Turn events
- **WHEN** a turn starts, receives deltas, calls tools, and completes
- **THEN** corresponding JSONL records are appended in order

#### Scenario: Write failure
- **WHEN** persistence write fails
- **THEN** the engine reports a non-fatal persistence warning unless strict mode is enabled

### Requirement: Session index SHALL support discovery
The system MUST maintain a TOML index for recent sessions.

#### Scenario: Session create
- **WHEN** a session is created
- **THEN** index records id, model, created time, message count, and optional summary

#### Scenario: Session list
- **WHEN** UI or CLI requests session list
- **THEN** entries are returned sorted by updated time descending

### Requirement: Persistence SHALL support resume and fork
Saved state MUST reconstruct usable engine sessions.

#### Scenario: Resume
- **WHEN** session id is loaded
- **THEN** messages and metadata are restored

#### Scenario: Fork
- **WHEN** a session is forked
- **THEN** a new id is assigned and original files are not modified

## Out of Scope for this phase

- Database backend.
- Encrypted logs.
