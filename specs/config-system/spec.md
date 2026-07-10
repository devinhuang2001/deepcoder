# Config System Spec

## Current State

`deepcoder-config` supports defaults, global config, project config, environment API key, top-level `api_key`, `[system]` settings, key lookup, redacted listing, and persistent writes through `Config::set_value_at_path`. CLI `config get/set/list` is wired to these APIs and tests cover layering, writes, invalid keys, and secret redaction.

## ADDED Requirements

### Requirement: Config SHALL load layered settings
Configuration MUST load in deterministic precedence order.

#### Scenario: Layer order
- **WHEN** DeepCoder starts
- **THEN** defaults are created first
- **THEN** global config overrides defaults
- **THEN** project config overrides global config
- **THEN** CLI flags override all file values

#### Scenario: API key source
- **WHEN** `--api-key` is provided
- **THEN** it overrides config files and `DEEPSEEK_API_KEY`
- **WHEN** no CLI key is provided
- **THEN** environment key may be used

### Requirement: Config schema SHALL be documented and stable
The config file MUST support provider, sandbox, UI, and system settings.

#### Scenario: Unknown or invalid values
- **WHEN** a config file has invalid TOML or invalid value type
- **THEN** startup fails with a clear config error

### Requirement: CLI config command SHALL manage user config
The CLI MUST provide get/set/list operations for common config keys.

#### Scenario: Config set
- **WHEN** user runs `deepcoder config set provider.model deepseek-chat`
- **THEN** global config is updated
- **THEN** future starts use the saved value

## Out of Scope for this phase

- Secret storage in OS keychains.
- Hot reload beyond explicit command-triggered reload.
