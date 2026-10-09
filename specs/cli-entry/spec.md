# CLI Entry Spec

## Current State

`deepcoder-cli` exposes default TUI mode plus `ui`, `exec`, `mcp-server`, `app-server`, and `config get/set/list`. It loads layered config, supports model/API-key overrides, provides `app-server --stdio`, preflights missing API keys for `exec`, and has unit plus binary-level mock-provider acceptance tests for `exec`.

## ADDED Requirements

### Requirement: CLI SHALL support primary run modes
The binary MUST route users to interactive, batch, MCP, and AppServer modes.

#### Scenario: Default interactive mode
- **WHEN** user runs `deepcoder` without subcommands
- **THEN** TUI mode starts

#### Scenario: Exec mode
- **WHEN** user runs `deepcoder exec "task"`
- **THEN** the engine runs one turn
- **THEN** stdout prints the final assistant text
- **THEN** stderr prints diagnostics only

#### Scenario: MCP server mode
- **WHEN** user runs `deepcoder mcp-server`
- **THEN** MCP stdio server starts

#### Scenario: AppServer mode
- **WHEN** user runs `deepcoder app-server --ws 127.0.0.1:8080`
- **THEN** AppServer starts on the requested address

### Requirement: CLI SHALL provide configuration commands
Users MUST be able to inspect and update common settings.

#### Scenario: Config list
- **WHEN** user runs `deepcoder config list`
- **THEN** effective config is printed with secrets redacted

#### Scenario: Config set
- **WHEN** user runs `deepcoder config set provider.model deepseek-chat`
- **THEN** global config is updated

### Requirement: CLI SHALL use clear exit codes
The process MUST return predictable exit codes for success, config errors, provider errors, and tool errors.

#### Scenario: Missing API key
- **WHEN** a command requires provider access and no API key exists
- **THEN** CLI exits non-zero
- **THEN** stderr explains how to set `DEEPSEEK_API_KEY`

## Out of Scope for this phase

- Shell completions.
- Self-update installer and shell completions.
