# DeepCoder 实施任务拆分

状态只使用 `Done`、`Partial`、`Todo`、`Blocked`。每个任务必须能追溯到对应 `specs/<module>/spec.md` 与 `specs/<module>/test-spec.md`。

实现参考来源见 `REFERENCE_SOURCES.md`：Codex zip 为 Apache-2.0，可直接适配；Claude Code zip 标注为未授权泄露源码，不得复制或派生，只能参考公开产品行为。

## Phase 0: 当前基线

| ID | 状态 | 目标 | 涉及模块 | 验收测试 |
| --- | --- | --- | --- | --- |
| P0-001 | Done | Cargo workspace 与 15 个核心 crate 可被 Cargo 识别 | workspace, cli, desktop, tui, engine, provider, tools, config, sandbox, app-server, mcp, persistence, skills, types, error | `cargo check --workspace` 通过 |
| P0-002 | Done | 修复基础编译错误并统一错误/类型依赖 | types, error, provider, tools, engine, tui, cli | `cargo check --workspace` 通过 |
| P0-003 | Done | 建立 DeepSeek Provider 基础 SSE 解析能力 | deepseek-v4-provider, reasoning-stream | mock SSE 测试覆盖 text/reasoning/tool_calls/DONE |
| P0-004 | Done | 建立基础 CLI 子命令入口 | cli-entry | `deepcoder --help`、`deepcoder exec <query>`、`mcp-server`、`app-server` 可解析 |
| P0-005 | Done | 建立基础 TUI 启动、输入、提交、错误展示 | tui-interface, core-engine | `app_initial_state`、`handle_key_appends_char_and_backspace`、`tui_streaming_updates_chat_and_reasoning`、`tui_provider_error_restores_input_state` 通过 |
| P0-006 | Done | 建立 AppServer/MCP 基础服务能力 | app-server, mcp-integration | AppServer/MCP 已具备 JSON-RPC/MCP 协议单测与 stdio/WebSocket/in-process 传输覆盖 |
| P0-007 | Done | 建立持久化、技能、沙箱基础 crate | state-persistence, skill-system, sandbox-security | JSONL/index/resume/fork、skill path filter、sandbox policy 单测通过 |
| P0-008 | Done | 记录 Windows 构建环境注意事项 | integration-release | 文档包含 `CARGO_TARGET_DIR` 与 `TMP/TEMP` 推荐设置 |

## Phase 1: P0 编码助手闭环

| ID | 状态 | 目标 | 涉及模块 | 验收测试 |
| --- | --- | --- | --- | --- |
| P1-001 | Done | 实现 `FileReadTool`，支持文本读取、行号、大小限制、路径错误 | tool-system | `file_tools_write_read_and_edit`、`file_tools_report_missing_limit_and_ambiguous_edits` 通过 |
| P1-002 | Done | 实现 `FileWriteTool`，支持创建/覆盖、父目录校验、错误返回 | tool-system | `file_tools_write_read_and_edit` 通过 |
| P1-003 | Done | 实现 `FileEditTool`，支持 exact string replacement 与 diff 输出 | tool-system | `file_tools_write_read_and_edit`、`file_tools_report_missing_limit_and_ambiguous_edits` 通过 |
| P1-004 | Done | 实现 `BashTool`，接入 `ExecPolicy`、timeout、stdout/stderr/exit code 与审批 | tool-system, sandbox-security, tui-interface | `bash_runs_allowed_command`、`bash_reports_timeout_and_denies_dangerous_command`、`bash_prompt_requires_explicit_approval`、`bash_uses_configured_sandbox_mode`、`tui_approver_sends_prompt_and_receives_denial` 通过 |
| P1-005 | Done | 实现 `GlobTool` 与 `GrepTool` | tool-system | `glob_and_grep_return_matches`、`grep_reports_invalid_regex` 通过 |
| P1-006 | Done | 实现 `WebFetchTool` 与 `WebSearchTool` 的可测试接口 | tool-system | `web_fetch_handles_mock_http_status_truncation_and_timeout`、`web_search_uses_mock_duckduckgo_results` 通过 |
| P1-007 | Done | 启动时注册内置工具并暴露 direct specs | tool-system, core-engine | `builtin_direct_specs_include_core_tools` 通过 |
| P1-008 | Done | 完成 tool loop：模型请求工具、执行工具、工具结果回灌、继续请求模型 | core-engine, deepseek-v4-provider | `tool_loop_sends_tool_result_back_to_provider` 通过 |
| P1-009 | Done | 支持多工具调用的顺序执行与并发安全执行策略 | core-engine, tool-system | `safe_tool_calls_execute_concurrently`、`unsafe_tool_calls_execute_sequentially` 通过 |
| P1-010 | Done | Provider 增加重试、退避、错误分类、坏 SSE 容错 | deepseek-v4-provider | `chat_stream_returns_classified_non_success_status`、`chat_stream_retries_retryable_status_then_streams`、`chat_stream_returns_api_error_when_connection_closes_before_response`、`stream_parser_covers_text_reasoning_tool_done_and_bad_json`、`stream_allows_eof_without_done_after_last_delta` 通过 |
| P1-011 | Done | CLI `exec` 输出最终助手文本并正确返回错误码 | cli-entry, core-engine | `exec_success_with_mock_provider`、`missing_api_key_error_is_actionable_from_binary`、`cli_parses_exec` 通过 |
| P1-012 | Done | TUI 实现后台流式刷新，不阻塞输入渲染 | tui-interface | `tui_streaming_updates_chat_and_reasoning`、`tui_provider_error_restores_input_state`、`widgets_render_without_panic` 通过 |

## Phase 2: P1 工程化能力

| ID | 状态 | 目标 | 涉及模块 | 验收测试 |
| --- | --- | --- | --- | --- |
| P2-001 | Done | 实现 JSONL 事件持久化并接入 turn lifecycle | state-persistence, core-engine | `engine_records_turn_events`、`tool_loop_sends_tool_result_back_to_provider`、`jsonl_append_event_writes_one_line` 通过 |
| P2-002 | Done | 实现 session index、resume、fork | state-persistence, core-engine, context-management | `persistence_resume_and_fork_reconstruct_messages`、`resume_and_fork_restore_usable_sessions`、`session_index_save_load_and_sort` 通过 |
| P2-003 | Done | 实现 token usage 记录与状态查询 | core-engine, context-management, tui-interface | `token_usage_accumulates_per_turn`、`context_budget_counts_sections` 通过 |
| P2-004 | Done | 实现 JSON-RPC MessageProcessor 与方法路由 | app-server | `parses_initialize_request`、`unknown_method_returns_json_rpc_error`、`invalid_json_returns_parse_error`、`missing_thread_id_returns_invalid_params` 通过 |
| P2-005 | Done | 实现 AppServer stdio、WebSocket、in-process channel 传输 | app-server, tui-interface | `stdio_transport_round_trip`、`websocket_transport_round_trip`、`inprocess_transport_round_trip` 通过 |
| P2-006 | Done | 实现线程 CRUD 与回合管理 API | app-server, core-engine, state-persistence | `thread_crud_round_trip`、`turn_start_streams_events`、`turn_cancel_records_cancel_request`、`turn_start_rejects_pre_cancelled_turn_id` 通过 |
| P2-007 | Done | 实现 MCP client 工具发现与 ToolRouter 注册 | mcp-integration, tool-system | `mcp_client_initialize_and_list_tools`、`mcp_client_registers_and_calls_tool` 通过 |
| P2-008 | Done | 实现 MCP server 暴露 DeepCoder 工具 | mcp-integration, tool-system | `initialize_returns_server_info`、`tools_list_includes_builtin_tools`、`tools_call_executes_builtin_tool`、`tools_call_missing_name_returns_invalid_params`、`unknown_method_returns_error` 通过 |
| P2-009 | Done | 完善技能系统 YAML frontmatter、路径匹配、prompt 注入 | skill-system, core-engine | `skills_parse_frontmatter_paths`、`skills_filter_by_active_paths`、`engine_injects_active_skills` 通过 |
| P2-010 | Done | 配置系统支持 `config` 子命令、配置写入、项目配置覆盖 | config-system, cli-entry | `config_set_persists_global_file`、`config_loads_global_and_project_file`、CLI `config get/set/list` 已接入 |

## Phase 3: P2 完整体验与发布

| ID | 状态 | 目标 | 涉及模块 | 验收测试 |
| --- | --- | --- | --- | --- |
| P3-001 | Done | 实现 Markdown、代码高亮、Diff 渲染 | tui-interface | `format_message_lines_styles_markdown_and_diff`、`widgets_render_without_panic` 通过 |
| P3-002 | Done | 实现多行输入、历史、SessionPicker、ApprovalOverlay | tui-interface, app-server, tool-system | `handle_key_supports_multiline_input`、`input_history_moves_up_and_down`、`session_picker_resumes_persisted_session`、`session_picker_renders_sessions`、`approval_prompt_accepts_and_advances_queue` 通过 |
| P3-003 | Done | 实现上下文压缩、摘要、长上下文预算策略 | context-management, core-engine | `context_keeps_recent_messages`、`context_preserves_tool_metadata`、`single_message_over_budget` 通过 |
| P3-004 | Done | 完成 ExtensionRegistry 六类贡献点与 ExtensionData 作用域 | extension-system | `registry_builder_empty`、`registry_builder_adds_contributors`、`extension_data_scopes_are_isolated`、`turn_hooks_are_async_object_safe` 通过 |
| P3-005 | Done | 完成平台沙箱：Linux/macOS/Windows 最小可用实现 | sandbox-security, tool-system | `sandbox_auto_reports_platform_backend`、`sandbox_enforce_rejects_dangerous_command`、`bash_uses_configured_sandbox_mode`、`prefix_rule_allows_known_command_with_boundary` 通过 |
| P3-006 | Done | 增加 AgentTool 与任务工具 | tool-system, core-engine | `agent_tool_spawns_isolated_task_contract`、`task_tools_create_list_update_and_delete` 通过 |
| P3-007 | Done | 建立 mock DeepSeek 集成测试套件 | deepseek-v4-provider, core-engine, tool-system | Provider/AppServer/Engine mock SSE 测试通过，`cargo test --workspace` 不依赖真实 API key |
| P3-008 | Done | 配置 CI：fmt、clippy、test、三平台 build | integration-release | `.github/workflows/ci.yml` 覆盖 fmt、clippy、test、三平台 build |
| P3-009 | Done | 配置 release 构建、校验、产物命名与 README 安装说明 | integration-release, cli-entry | `.github/workflows/release.yml`、`release_artifact_name_format`、`checksum_file_format`、`cli_exec_binary_mock_provider` 已完成 |

## Phase 4: Windows 桌面版与小白启动

| ID | 状态 | 目标 | 涉及模块 | 验收测试 |
| --- | --- | --- | --- | --- |
| P4-001 | Done | 新增 Rust `eframe/egui` Windows 桌面 app | desktop-interface, core-engine | `cargo test -p deepcoder-desktop`、`cargo build -p deepcoder-desktop` 通过 |
| P4-002 | Done | 实现 Codex 风格聊天工作台：会话列表、聊天区、推理/工具详情、输入区 | desktop-interface, state-persistence, reasoning-stream | `reducer_applies_stream_events`、`spawn_turn_streams_mock_provider_events` 通过 |
| P4-003 | Done | 实现首次启动 API Key 配置向导并写入现有 config | desktop-interface, config-system | `config_wizard_required_when_key_missing`、`save_wizard_config_updates_file_and_current_config` 通过 |
| P4-004 | Done | 实现桌面工具审批弹窗与后台 approver 通路 | desktop-interface, tool-system | `approval_flow_returns_user_decision`、`desktop_approver_round_trips_decision` 通过 |
| P4-005 | Done | 新增一键启动与构建脚本，放在项目根目录 | desktop-interface, integration-release | `启动DeepCoder.bat`、`启动DeepCoder.ps1`、`构建桌面版.ps1` 已添加 |
| P4-006 | Done | 新增桌面版 spec/test-spec 并更新 README 小白启动说明 | desktop-interface, docs | `specs/desktop-interface/spec.md`、`specs/desktop-interface/test-spec.md` 已添加 |

## Phase 5: 工程加固与真实 Web 闭环

| ID | 状态 | 目标 | 涉及模块 | 验收测试 |
| --- | --- | --- | --- | --- |
| P5-001 | Done | 增加 `.gitignore`，避免提交 `target`、`node_modules`、`dist`、本地 exe | workspace, integration-release | 根目录 `.gitignore` 已覆盖构建产物与本地二进制 |
| P5-002 | Done | Provider 支持分片 `tool_calls[].function.arguments` 累积，并对 loopback mock URL 禁用代理 | deepseek-v4-provider | `stream_parser_accumulates_chunked_tool_call_until_done`、`loopback_base_url_detection_covers_local_mock_hosts` 通过 |
| P5-003 | Done | 文件/搜索工具限制路径在 workspace 内，拒绝绝对路径和 `..` 逃逸 | tool-system, sandbox-security | `file_tools_restrict_paths_to_workspace_root`、`search_tools_restrict_paths_to_workspace_root` 通过 |
| P5-004 | Done | AppServer WebSocket 支持 `turn/event` 实时通知，同时保留最终 JSON-RPC response | app-server, core-engine | `turn_start_streams_events` 覆盖 response events 与 notification events |
| P5-005 | Done | Web 前端接入 AppServer WebSocket，不再使用模拟回复 | app-server, web-interface | `npm run build` 通过，前端处理 `initialize`、`thread/create`、`turn/start`、`turn/event` |
| P5-006 | Done | CI 增加 Web build，release Windows 包增加桌面 exe 与一键启动脚本 | integration-release, desktop-interface, web-interface | `.github/workflows/ci.yml`、`.github/workflows/release.yml` 已更新 |
| P5-007 | Done | 平台 sandbox 状态与真实能力对齐，不再声称未实现的 OS 级 backend 可用 | sandbox-security | `sandbox_auto_reports_platform_backend`、`sandbox_auto_still_blocks_dangerous_command_without_backend` 通过 |
| P5-008 | Done | WebSocket `turn/cancel` 可抢占运行中的 turn，Web 停止按钮接入真实取消 | app-server, web-interface, core-engine | `websocket_turn_cancel_aborts_running_turn`、`npm run build` 通过 |
| P5-009 | Done | Web 侧边栏使用真实 `thread/list` 数据并支持 `thread/create` 新建会话 | app-server, web-interface, state-persistence | `npm run build` 通过；启动后恢复已有线程，没有则创建新线程 |
| P5-010 | Done | AppServer `thread/get` 返回持久化 messages，Web 切换会话恢复历史对话 | app-server, web-interface, state-persistence | `thread_get_returns_persisted_messages`、`npm run build` 通过 |
| P5-011 | Done | Web 右侧面板显示真实 reasoning、工具事件和 token usage | web-interface, app-server, reasoning-stream | `npm run build` 通过；`turn/event` 与历史 messages 均可驱动右侧面板 |
| P5-012 | Done | 新增 Web 工作台一键启动脚本，并将 Web 源文件纳入 Windows release 包 | web-interface, app-server, integration-release | `StartDeepCoderWeb.ps1 -ValidateOnly`、`npm run build` 通过；release workflow 复制 Web launcher 与 `deepcoder-web` |
| P5-013 | Done | 为 Web 工作台补齐 `specs/web-interface/spec.md` 与 `test-spec.md`，恢复任务可追溯性 | web-interface, integration-release | `specs/web-interface/spec.md`、`specs/web-interface/test-spec.md` 存在，README 架构树已更新 |
| P5-014 | Done | 抽出 Web 状态转换 helper 并新增 Node 单元测试，CI 执行 `npm test` | web-interface, integration-release | `npm test` 覆盖 message bubble、diagnostics、thread transcript、turn id helper；CI web job 已加入 test 步骤 |
| P5-015 | Done | 重构 Web 工作台视觉系统，优化桌面三栏、移动端顶栏和输入区 | web-interface | `npm test`、`npm run build` 通过；Playwright + Edge 生成 1440x900 与 390x844 截图验证无明显空白、错位或文字溢出 |

## Phase 6: 安全公网部署与正式上线

| ID | 状态 | 目标 | 涉及模块 | 验收测试 |
| --- | --- | --- | --- | --- |
| P6-001 | Done | AppServer 增加强令牌、精确 Origin、协议版本、连接/消息/回合限制和生产 fail-closed | app-server, sandbox-security | `websocket_handshake_enforces_origin_and_token`、`websocket_rate_limit_closes_with_policy_violation`、`websocket_rejects_binary_json_rpc_frames_with_close_1003` 通过 |
| P6-002 | Done | Web 增加内存令牌登录、同源 WSS 与指数退避重连 | web-interface | `create_websocket_protocols_keeps_token_in_memory_only`、`resolve_websocket_url_uses_secure_same_origin_endpoint_in_production`、`calculate_reconnect_delay_uses_capped_exponential_backoff` 通过 |
| P6-003 | Done | 建立非 root 单容器、Caddy 静态/WS 反代、安全头、健康检查和持久目录 | integration-release, app-server, web-interface | `Dockerfile`、`deploy/Caddyfile`、`deploy/entrypoint.sh`、`render.yaml` 已添加；CI 构建镜像 |
| P6-004 | Done | 配置数据目录环境覆盖、生产 Web 资产打包与容器 CI | config-system, integration-release | `runtime_environment_overrides_data_directory`、`npm test`、`npm run build` 通过 |
| P6-005 | Partial | 同步 GitHub、合并 CI、发布首个正式 Release 并完成公网健康/登录/对话验收 | integration-release, ops | 等待远端 CI、Release URL、Render URL 与真实 DeepSeek key 验收 |

## 验证命令

Windows 当前路径包含中文，MinGW 构建时建议使用 D 盘 target/temp：

```powershell
New-Item -ItemType Directory -Force -Path D:\deepcoder-target,D:\codex-temp | Out-Null
$env:CARGO_TARGET_DIR='D:\deepcoder-target'
$env:TEMP='D:\codex-temp'
$env:TMP='D:\codex-temp'
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo test --workspace
cargo build --release -p deepcoder-desktop
cd ..\deepcoder-web
npm test
npm run build
```
