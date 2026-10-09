# Integration and Release Spec

## Current State

The workspace has CI for `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, Web build, production Docker build, and three-platform builds. Release workflow builds tagged `v*` artifacts, packages platform-specific binaries, includes Windows desktop and Web one-click launchers, includes `deepcoder-web` sources and production assets, and emits `.sha256` files. Docker/Caddy/Render files define the single-user production Web service, persistent disk, health check, and secret contract. Binary signing/notarization is intentionally out of scope.

## ADDED Requirements

### Requirement: Upstream source usage SHALL respect licenses
DeepCoder MUST only copy or derive code from sources with compatible licenses.

#### Scenario: Codex source adaptation
- **WHEN** implementation is adapted from `codex-main.zip`
- **THEN** the source license MUST be treated as Apache-2.0
- **THEN** attribution and license obligations MUST be preserved

#### Scenario: Unlicensed source package
- **WHEN** a source package declares itself unlicensed or leaked proprietary code
- **THEN** DeepCoder MUST NOT copy, rewrite, translate, or derive implementation from it
- **THEN** only public product behavior may be used as high-level reference

### Requirement: CI SHALL verify formatting, linting, tests, and builds
Every pull request MUST run a reproducible validation matrix.

#### Scenario: Formatting
- **WHEN** CI runs
- **THEN** `cargo fmt --check` passes

#### Scenario: Linting
- **WHEN** CI runs clippy
- **THEN** `cargo clippy --workspace --all-targets -- -D warnings` passes

#### Scenario: Tests
- **WHEN** CI runs tests
- **THEN** `cargo test --workspace` passes without live API keys

#### Scenario: Web tests
- **WHEN** CI runs the Web job
- **THEN** `npm test` passes before `npm run build`

#### Scenario: Platform builds
- **WHEN** CI matrix runs
- **THEN** Windows, macOS, and Linux builds pass

#### Scenario: Production container
- **WHEN** CI runs on a pull request or main
- **THEN** the root Dockerfile builds the authenticated Web, Rust AppServer, and Caddy runtime
- **THEN** no API key or access token is required at image build time

### Requirement: Release SHALL produce installable artifacts
Tagged releases MUST build, package, and publish platform-specific binaries.

#### Scenario: Artifact naming
- **WHEN** release builds complete
- **THEN** artifacts include OS, architecture, version, and checksum

#### Scenario: Install guide
- **WHEN** release is published
- **THEN** README or release notes explain download, config, and API key setup

#### Scenario: Windows one-click launchers
- **WHEN** Windows release artifact is packaged
- **THEN** it includes desktop launcher scripts
- **THEN** it includes Web launcher scripts
- **THEN** it includes `deepcoder-web` source files required by the Web launcher
- **THEN** it includes prebuilt `deepcoder-web/dist` assets

### Requirement: Local validation SHALL document Windows caveats
Docs MUST explain target/temp directory overrides for paths and disk space.

#### Scenario: Windows MinGW
- **WHEN** local path contains non-ASCII characters or C temp is full
- **THEN** docs recommend D drive `CARGO_TARGET_DIR`, `TMP`, and `TEMP`

## Out of Scope for this phase

- Package manager publishing.
- Binary signing notarization beyond documenting future requirement.
