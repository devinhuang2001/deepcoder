# MCP Integration Spec

## Current State

`deepcoder-mcp` implements a stdio MCP server with `initialize`, `tools/list`, and `tools/call` over built-in DeepCoder tools. The client side can initialize a remote MCP server, list tools, register discovered tools into `ToolRouter`, and call them through a dynamic `Tool` wrapper. Resource discovery and OAuth are not in this phase.

## ADDED Requirements

### Requirement: MCP client SHALL consume external MCP servers
DeepCoder MUST connect to configured MCP servers and register their tools.

#### Scenario: Initialize server
- **WHEN** MCP server config is present
- **THEN** client sends initialize
- **THEN** capabilities are recorded

#### Scenario: Tool discovery
- **WHEN** server returns tools/list
- **THEN** each MCP tool is wrapped as a DeepCoder tool
- **THEN** ToolRouter can dispatch it

#### Scenario: Tool call
- **WHEN** model calls a wrapped MCP tool
- **THEN** DeepCoder sends MCP tools/call
- **THEN** response content is converted to `JsonToolOutput`

### Requirement: MCP server SHALL expose DeepCoder tools
DeepCoder MUST act as an MCP server for external clients.

#### Scenario: tools/list
- **WHEN** an MCP client requests tools/list
- **THEN** direct DeepCoder tools are returned with schemas

#### Scenario: tools/call
- **WHEN** an MCP client calls a tool
- **THEN** ToolRouter executes it
- **THEN** MCP response includes success or error content

### Requirement: MCP errors SHALL be protocol compliant
MCP failures MUST return structured errors without panics.

#### Scenario: Unknown tool
- **WHEN** MCP client calls unknown tool
- **THEN** server returns a tool-not-found error

## Out of Scope for this phase

- OAuth implementation beyond the current local tool bridge.
- MCP resources and prompts beyond tool discovery.
