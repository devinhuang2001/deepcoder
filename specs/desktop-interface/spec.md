# Desktop Interface Spec

## Current State

`deepcoder-desktop` provides a Rust `eframe/egui` Windows desktop app that launches independently from the existing CLI/TUI. It directly uses `deepcoder-engine`, renders a Codex-style chat workspace, streams engine events into the UI, supports tool approval prompts, lists persisted sessions, and includes a first-run API Key configuration wizard that writes to the existing DeepCoder config file.

## ADDED Requirements

### Requirement: Desktop app SHALL provide a small-user-friendly launcher UI
The desktop app MUST open a native window titled `DeepCoder` and show a chat workspace without requiring terminal knowledge.

#### Scenario: App startup
- **WHEN** `deepcoder-desktop` starts
- **THEN** a desktop window opens with top status, session list, chat area, details panel, and input area

#### Scenario: Missing API key
- **WHEN** neither config `api_key` nor `DEEPSEEK_API_KEY` exists
- **THEN** the first-run configuration wizard is shown before chat submission

### Requirement: Desktop app SHALL run engine turns directly
The desktop app MUST call `deepcoder-engine` in a background runtime and keep the UI responsive.

#### Scenario: Streaming text
- **WHEN** engine emits `TextDelta`
- **THEN** the assistant message updates in the chat area

#### Scenario: Reasoning stream
- **WHEN** engine emits `ReasoningDelta`
- **THEN** the reasoning panel updates

#### Scenario: Turn complete
- **WHEN** engine emits `TurnComplete`
- **THEN** token usage updates and the input becomes usable again

### Requirement: Desktop app SHALL expose approval and session workflows
Tool prompts and persisted sessions MUST be usable without command-line interaction.

#### Scenario: Tool approval
- **WHEN** a tool returns `PermissionResult::Prompt`
- **THEN** the desktop app shows an approval dialog
- **THEN** approving returns true to the waiting tool
- **THEN** denying returns false

#### Scenario: Session resume
- **WHEN** persisted sessions exist
- **THEN** the left panel lists them
- **THEN** user can resume or fork the selected session

## Out of Scope for this phase

- Installer/MSIX packaging.
- Windows Credential Manager storage.
- Pixel-perfect clone of Codex product UI.
