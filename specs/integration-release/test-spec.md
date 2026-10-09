# Integration and Release Test Spec

## Unit Tests

- `release_artifact_name_format`
  - 准备：version、target triple。
  - 执行：生成 artifact name。
  - 断言：包含 binary name、version、os、arch。
  - Mock：不需要。
- `checksum_file_format`
  - 准备：artifact bytes。
  - 执行：生成 checksum。
  - 断言：输出 sha256 和文件名。
  - Mock：临时文件。

## Integration Tests

- `workspace_fmt_check`
  - 准备：repo clean enough for check。
  - 执行：`cargo fmt --check`。
  - 断言：退出码 0。
  - Mock：不需要。
- `workspace_clippy`
  - 准备：依赖已安装。
  - 执行：`cargo clippy --workspace --all-targets -- -D warnings`。
  - 断言：退出码 0。
  - Mock：不需要。
- `workspace_test_no_live_api`
  - 准备：不设置真实 API key。
  - 执行：`cargo test --workspace`。
  - 断言：退出码 0，所有 provider tests 使用 mock。
  - Mock：mock HTTP/SSE。
- `web_unit_tests`
  - 准备：`deepcoder-web` 依赖已安装。
  - 执行：`npm test`。
  - 断言：Node test runner 通过 Web helper 单元测试。
  - Mock：不需要。
- `web_ci_runs_tests_before_build`
  - 准备：`.github/workflows/ci.yml`。
  - 执行：静态检查 web job。
  - 断言：`npm test` 步骤位于 `npm run build` 前。
  - Mock：不需要。
- `production_container_build`
  - 准备：Docker daemon 与根目录构建上下文。
  - 执行：`docker build -t deepcoder:test .`。
  - 断言：多阶段 Rust/Web/Caddy 镜像构建成功，构建期不要求 secrets。
  - Mock：不需要。
- `production_container_health_and_auth`
  - 准备：用测试 API key、Origin 和强访问令牌启动容器。
  - 执行：请求 `/health`，再发起缺令牌/正确令牌 WebSocket 握手。
  - 断言：health 返回 200；缺令牌失败；正确握手成功。
  - Mock：mock provider 或只验证 initialize。
- `release_build_cli`
  - 准备：设置推荐 `CARGO_TARGET_DIR`、`TMP`、`TEMP`。
  - 执行：`cargo build --release -p deepcoder-cli`.
  - 断言：release binary 存在。
  - Mock：不需要。
- `web_launcher_validate_only`
  - 准备：项目根目录包含 `deepcoder` workspace 和 `deepcoder-web/package.json`。
  - 执行：`powershell -ExecutionPolicy Bypass -File .\StartDeepCoderWeb.ps1 -ValidateOnly`。
  - 断言：退出码 0，不启动后台进程，不打开浏览器。
  - Mock：不需要。
- `windows_web_launcher_files_exist`
  - 准备：repo root。
  - 执行：静态检查根目录脚本与 `deepcoder-web` 必要文件。
  - 断言：`StartDeepCoderWeb.ps1`、`启动DeepCoder网页版.bat`、`启动DeepCoder网页版.ps1`、`deepcoder-web/package.json`、`deepcoder-web/src/App.jsx` 存在。
  - Mock：不需要。
- `windows_release_workflow_packages_web_launcher`
  - 准备：Windows release workflow package step。
  - 执行：静态检查 `.github/workflows/release.yml`。
  - 断言：复制 `StartDeepCoderWeb.ps1`、`启动DeepCoder网页版.bat`、`启动DeepCoder网页版.ps1` 和 `deepcoder-web` 源文件。
  - Mock：不需要。

## CLI/TUI Acceptance Tests

- `installed_binary_help`
  - 准备：release binary。
  - 执行：`deepcoder --help`。
  - 断言：命令列表与 README 一致。
  - Mock：不需要。
- `cli_exec_binary_mock_provider`
  - 准备：debug/test binary、临时 config、本地 mock SSE provider。
  - 执行：`deepcoder --config <path> exec "say hello"`。
  - 断言：退出码 0，stdout 包含 mock 文本。
  - Mock：本地 HTTP/SSE。
- `readme_quickstart_matches_cli`
  - 准备：README quickstart commands。
  - 执行：静态检查命令名称。
  - 断言：README 不引用不存在的 subcommand。
  - Mock：不需要。

## Failure Cases

- `windows_temp_space_documented`
  - 准备：README/BUILD/tasks 文档。
  - 执行：搜索 `CARGO_TARGET_DIR`、`TMP`、`TEMP`。
  - 断言：Windows workaround 存在且命令可复制。
  - Mock：不需要。
- `ci_missing_api_key_does_not_fail_mock_tests`
  - 准备：CI 环境无 `DEEPSEEK_API_KEY`。
  - 执行：测试套件。
  - 断言：mock tests 通过，只有 live tests 被跳过。
  - Mock：mock provider。
