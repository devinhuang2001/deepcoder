# TUI Interface Spec

## Current State

`deepcoder-tui` starts a ratatui terminal, renders chat/reasoning/input/status sections, runs engine turns in a background task, drains streamed events without blocking redraw, renders basic Markdown/code/diff styling, supports multiline input, history navigation, approval overlay, and a persisted session picker with resume/fork actions. Tests cover key handling, stream updates, provider errors, approval flow, session picker, and widget rendering.

## ADDED Requirements

### Requirement: TUI SHALL provide an interactive chat surface
The TUI MUST render user input, assistant output, reasoning, and status without corrupting terminal state.

#### Scenario: Startup and exit
- **WHEN** TUI starts
- **THEN** terminal raw mode is initialized
- **WHEN** user exits
- **THEN** terminal state is restored

#### Scenario: Message display
- **WHEN** user submits input
- **THEN** user message is displayed
- **THEN** assistant deltas append to a separate assistant message

### Requirement: TUI SHALL stream without blocking rendering
Model events MUST update the UI while the event loop continues drawing.

#### Scenario: Text delta
- **WHEN** engine emits `TextDelta`
- **THEN** chat area updates before turn completion

#### Scenario: Reasoning delta
- **WHEN** engine emits `ReasoningDelta`
- **THEN** reasoning panel updates if visible

#### Scenario: Error event
- **WHEN** engine emits `Error`
- **THEN** status line and chat area show the error
- **THEN** input becomes usable again

### Requirement: TUI SHALL support coding workflows
The TUI MUST render Markdown, code blocks, diffs, approvals, and session selection.

#### Scenario: Diff render
- **WHEN** a tool returns diff data
- **THEN** added and removed lines are visually distinct

#### Scenario: Approval overlay
- **WHEN** a tool needs prompt approval
- **THEN** user can approve or deny before execution

#### Scenario: Session picker
- **WHEN** persisted sessions exist
- **THEN** user can resume or fork a session

## Out of Scope for this phase

- Mouse-first workflow.
- Rich image rendering in terminal.
