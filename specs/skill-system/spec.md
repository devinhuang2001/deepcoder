# Skill System Spec

## Current State

`deepcoder-skills` scans configured skill roots for `SKILL.md`, parses scalar/list/inline-array frontmatter, warns and continues on invalid skills, filters active skills by path globs, and builds stable sorted/deduped prompt injection text. Engine turn construction injects active skills from project and data-dir skill roots.

## ADDED Requirements

### Requirement: Skills SHALL load from SKILL.md files
The system MUST discover skill directories and parse metadata plus instructions.

#### Scenario: Skill discovery
- **WHEN** a skill directory contains `SKILL.md`
- **THEN** name, description, optional paths, and instructions are loaded

#### Scenario: Invalid skill file
- **WHEN** a `SKILL.md` cannot be parsed
- **THEN** loading continues for other skills
- **THEN** a warning is logged

### Requirement: Skills SHALL be selected by context
The system MUST activate skills based on user request and active file paths.

#### Scenario: Path filter
- **WHEN** a skill declares path filters
- **THEN** it activates only when active paths match

#### Scenario: No path filter
- **WHEN** a skill declares no paths
- **THEN** it may activate based on trigger/description matching

### Requirement: Skills SHALL inject prompt content deterministically
Active skills MUST be added to the system prompt in stable order.

#### Scenario: Prompt injection
- **WHEN** active skills are selected
- **THEN** prompt contains skill names, descriptions, and instructions
- **THEN** duplicate skills are not injected

## Out of Scope for this phase

- Downloading remote skills.
- Skill marketplace or plugin install UX.
