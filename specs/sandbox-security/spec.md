# Sandbox Security Spec

## Current State

`deepcoder-sandbox` has `SandboxManager`, platform type detection for Linux/macOS/Windows/none, dangerous command pattern checks, process hardening hooks, and an `ExecPolicy` prefix-rule engine with boundary-safe matching. `BashTool` evaluates prompt/allow/deny policy before execution and checks `ctx.config.sandbox.mode` immediately before spawning. Real OS-level sandbox backends are not enabled yet; `auto` keeps policy checks and dangerous-command fail-closed behavior, while `enforce` fails if no backend is available.

## ADDED Requirements

### Requirement: ExecPolicy SHALL gate shell execution
Shell commands MUST be evaluated before execution.

#### Scenario: Allowed prefix
- **WHEN** command starts with an allow rule prefix
- **THEN** policy returns allow

#### Scenario: Denied command
- **WHEN** command matches deny rule or dangerous pattern
- **THEN** execution is blocked
- **THEN** the tool returns a denied error

#### Scenario: Prompt decision
- **WHEN** no rule matches
- **THEN** policy returns prompt
- **THEN** TUI/AppServer approval flow decides execution

### Requirement: Platform sandbox SHALL match OS capability
The system MUST select a sandbox backend based on OS and config mode.

#### Scenario: Auto mode
- **WHEN** sandbox mode is `auto`
- **THEN** platform backend is selected if available
- **THEN** unsupported platforms run without platform sandbox but keep policy checks

#### Scenario: Enforce mode
- **WHEN** sandbox mode is `enforce` and backend is unavailable
- **THEN** shell execution fails closed

### Requirement: Process hardening SHALL run at startup
Startup hardening MUST remove dangerous dynamic linker variables and apply platform guards where supported.

#### Scenario: Linux hardening
- **WHEN** process starts on Linux
- **THEN** dangerous `LD_*` variables are removed

#### Scenario: Windows hardening
- **WHEN** process starts on Windows
- **THEN** handle inheritance and process restrictions are applied where available

## Out of Scope for this phase

- Perfect containment for all OS-level attacks.
- Network sandboxing beyond shell policy.
