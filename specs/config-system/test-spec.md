# Config System Test Spec

## Unit Tests

- `config_default_values`
  - 准备：无配置文件、无环境变量。
  - 执行：加载默认 config。
  - 断言：provider type/model/base_url、sandbox mode、ui defaults 正确。
  - Mock：临时 HOME/config dir。
- `config_loads_global_file`
  - 准备：临时全局 `config.toml`。
  - 执行：加载 config。
  - 断言：文件值覆盖 defaults。
  - Mock：临时 config dir。
- `config_loads_project_file`
  - 准备：临时项目 `.deepcoder/config.toml`。
  - 执行：在项目目录加载 config。
  - 断言：项目配置覆盖全局配置。
  - Mock：临时目录。
- `config_env_api_key`
  - 准备：设置 `DEEPSEEK_API_KEY`。
  - 执行：加载 config。
  - 断言：config.api_key 为环境值。
  - Mock：环境变量隔离。

## Integration Tests

- `cli_overrides_config`
  - 准备：文件配置 model=A，CLI 传 model=B。
  - 执行：CLI config loading path。
  - 断言：最终 model=B。
  - Mock：临时 config dir。
- `config_set_persists_global_file`
  - 准备：临时 config dir。
  - 执行：`deepcoder config set provider.model deepseek-chat`。
  - 断言：config file 被写入，重新加载后生效。
  - Mock：临时 config dir。

## CLI/TUI Acceptance Tests

- `config_list_redacts_secrets`
  - 准备：配置 API key。
  - 执行：`deepcoder config list`。
  - 断言：输出不包含完整 secret，非 secret 字段可见。
  - Mock：临时 config dir。

## Failure Cases

- `invalid_toml_returns_context`
  - 准备：写入非法 TOML。
  - 执行：加载 config。
  - 断言：返回 config error，包含文件路径。
  - Mock：临时 config dir。
- `invalid_key_rejected`
  - 准备：无。
  - 执行：`deepcoder config set unknown.key value`。
  - 断言：非零退出码，错误提示合法 key。
  - Mock：临时 config dir。
