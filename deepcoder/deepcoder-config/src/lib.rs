//! DeepCoder 配置系统
//!
//! TOML 分层配置：系统默认 → 全局配置 → 项目配置 → CLI 覆盖

use anyhow::{Context, Result};
use std::path::PathBuf;
use toml::Value;

/// 配置管理器
#[derive(Debug, Clone)]
pub struct Config {
    pub provider: ProviderConfig,
    pub sandbox: SandboxConfig,
    pub ui: UiConfig,
    pub system: SystemConfig,
    /// API Key（来自环境变量或配置文件）
    pub api_key: Option<String>,
}

/// Provider 配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderConfig {
    pub r#type: String,
    pub model: String,
    pub base_url: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            r#type: "deepseek".into(),
            model: "deepseek-chat".into(),
            base_url: "https://api.deepseek.com".into(),
            temperature: None,
            max_tokens: None,
        }
    }
}

/// 沙箱配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxConfig {
    pub mode: String,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            mode: "auto".into(),
        }
    }
}

/// UI 配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub show_reasoning: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "default".into(),
            show_reasoning: true,
        }
    }
}

/// 系统配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemConfig {
    pub data_dir: PathBuf,
    pub max_tool_iterations: u32,
    pub max_context_tokens: u32,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("deepcoder"),
            max_tool_iterations: 25,
            max_context_tokens: 131_072,
        }
    }
}

impl Config {
    /// 加载配置（默认值 + 全局配置 + 项目配置 + 环境变量）
    pub fn load_default() -> Result<Self> {
        Self::load_from_paths(
            Self::global_config_path(),
            PathBuf::from(".deepcoder/config.toml"),
        )
    }

    pub fn global_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("deepcoder")
            .join("config.toml")
    }

    pub fn load_from_paths(global_path: PathBuf, project_path: PathBuf) -> Result<Self> {
        let mut config = Config {
            provider: ProviderConfig::default(),
            sandbox: SandboxConfig::default(),
            ui: UiConfig::default(),
            system: SystemConfig::default(),
            api_key: None,
        };

        // 尝试加载全局配置
        if global_path.exists() {
            let content = std::fs::read_to_string(&global_path)
                .context(format!("读取配置失败: {}", global_path.display()))?;
            let file_config: ConfigFile = toml::from_str(&content)
                .context(format!("解析配置失败: {}", global_path.display()))?;
            config.apply_file(file_config);
        }

        // 尝试加载项目配置
        if project_path.exists() {
            let content = std::fs::read_to_string(&project_path)?;
            let file_config: ConfigFile = toml::from_str(&content)?;
            config.apply_file(file_config);
        }

        config.apply_environment_overrides_with(read_unicode_environment)?;

        Ok(config)
    }

    fn apply_environment_overrides_with<F>(&mut self, mut read: F) -> Result<()>
    where
        F: FnMut(&str) -> Result<Option<String>>,
    {
        if let Some(api_key) = read("DEEPSEEK_API_KEY")? {
            if api_key.trim().is_empty() {
                anyhow::bail!("DEEPSEEK_API_KEY cannot be empty when set");
            }
            self.api_key = Some(api_key);
        }
        if let Some(data_dir) = read("DEEPCODER_DATA_DIR")? {
            let data_dir = data_dir.trim();
            if data_dir.is_empty() {
                anyhow::bail!("DEEPCODER_DATA_DIR cannot be empty when set");
            }
            self.system.data_dir = PathBuf::from(data_dir);
        }
        Ok(())
    }

    pub fn get_value(&self, key: &str) -> Option<String> {
        match key {
            "provider.type" => Some(self.provider.r#type.clone()),
            "provider.model" => Some(self.provider.model.clone()),
            "provider.base_url" => Some(self.provider.base_url.clone()),
            "provider.temperature" => self.provider.temperature.map(|value| value.to_string()),
            "provider.max_tokens" => self.provider.max_tokens.map(|value| value.to_string()),
            "sandbox.mode" => Some(self.sandbox.mode.clone()),
            "ui.theme" => Some(self.ui.theme.clone()),
            "ui.show_reasoning" => Some(self.ui.show_reasoning.to_string()),
            "system.data_dir" => Some(self.system.data_dir.display().to_string()),
            "system.max_tool_iterations" => Some(self.system.max_tool_iterations.to_string()),
            "system.max_context_tokens" => Some(self.system.max_context_tokens.to_string()),
            "api_key" => self.api_key.as_ref().map(|value| redact_secret(value)),
            _ => None,
        }
    }

    pub fn list_values_redacted(&self) -> Vec<(String, String)> {
        supported_keys()
            .into_iter()
            .filter_map(|key| self.get_value(key).map(|value| (key.to_string(), value)))
            .collect()
    }

    pub fn set_global_value(key: &str, value: &str) -> Result<()> {
        Self::set_value_at_path(&Self::global_config_path(), key, value)
    }

    pub fn set_value_at_path(path: &std::path::Path, key: &str, value: &str) -> Result<()> {
        let mut root = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            content
                .parse::<Value>()
                .context(format!("解析配置失败: {}", path.display()))?
        } else {
            Value::Table(Default::default())
        };
        let value = parse_config_value(key, value)?;
        set_nested_value(&mut root, key, value)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(&root)?)?;
        Ok(())
    }

    fn apply_file(&mut self, file: ConfigFile) {
        if let Some(api_key) = file.api_key {
            self.api_key = Some(api_key);
        }
        if let Some(provider) = file.provider {
            if let Some(r#type) = provider.r#type {
                self.provider.r#type = r#type;
            }
            if let Some(model) = provider.model {
                self.provider.model = model;
            }
            if let Some(base_url) = provider.base_url {
                self.provider.base_url = base_url;
            }
            if provider.temperature.is_some() {
                self.provider.temperature = provider.temperature;
            }
            if provider.max_tokens.is_some() {
                self.provider.max_tokens = provider.max_tokens;
            }
        }
        if let Some(sandbox) = file.sandbox {
            self.sandbox.mode = sandbox.mode.unwrap_or(self.sandbox.mode.clone());
        }
        if let Some(ui) = file.ui {
            if let Some(theme) = ui.theme {
                self.ui.theme = theme;
            }
            if let Some(show) = ui.show_reasoning {
                self.ui.show_reasoning = show;
            }
        }
        if let Some(system) = file.system {
            if let Some(data_dir) = system.data_dir {
                self.system.data_dir = data_dir;
            }
            if let Some(max_tool_iterations) = system.max_tool_iterations {
                self.system.max_tool_iterations = max_tool_iterations;
            }
            if let Some(max_context_tokens) = system.max_context_tokens {
                self.system.max_context_tokens = max_context_tokens;
            }
        }
    }
}

fn read_unicode_environment(key: &str) -> Result<Option<String>> {
    match std::env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("{key} must contain valid Unicode")
        }
    }
}

/// 配置文件的序列化结构
#[derive(Debug, serde::Deserialize)]
struct ConfigFile {
    api_key: Option<String>,
    provider: Option<ProviderFileConfig>,
    sandbox: Option<SandboxFileConfig>,
    ui: Option<UiFileConfig>,
    system: Option<SystemFileConfig>,
}

#[derive(Debug, serde::Deserialize)]
struct ProviderFileConfig {
    r#type: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

#[derive(Debug, serde::Deserialize)]
struct SandboxFileConfig {
    mode: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct UiFileConfig {
    theme: Option<String>,
    show_reasoning: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct SystemFileConfig {
    data_dir: Option<PathBuf>,
    max_tool_iterations: Option<u32>,
    max_context_tokens: Option<u32>,
}

fn supported_keys() -> Vec<&'static str> {
    vec![
        "provider.type",
        "provider.model",
        "provider.base_url",
        "provider.temperature",
        "provider.max_tokens",
        "sandbox.mode",
        "ui.theme",
        "ui.show_reasoning",
        "system.data_dir",
        "system.max_tool_iterations",
        "system.max_context_tokens",
        "api_key",
    ]
}

fn parse_config_value(key: &str, value: &str) -> Result<Value> {
    match key {
        "provider.type" | "provider.model" | "provider.base_url" | "sandbox.mode" | "ui.theme"
        | "system.data_dir" | "api_key" => Ok(Value::String(value.to_string())),
        "provider.temperature" => Ok(Value::Float(value.parse()?)),
        "provider.max_tokens" | "system.max_tool_iterations" | "system.max_context_tokens" => {
            Ok(Value::Integer(value.parse()?))
        }
        "ui.show_reasoning" => Ok(Value::Boolean(value.parse()?)),
        _ => anyhow::bail!(
            "未知配置键: {key}. 支持的键: {}",
            supported_keys().join(", ")
        ),
    }
}

fn set_nested_value(root: &mut Value, key: &str, value: Value) -> Result<()> {
    let parts = key.split('.').collect::<Vec<_>>();
    let mut current = root
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("配置根必须是 TOML table"))?;
    for part in &parts[..parts.len().saturating_sub(1)] {
        let entry = current
            .entry((*part).to_string())
            .or_insert_with(|| Value::Table(Default::default()));
        current = entry
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("配置路径不是 table: {part}"))?;
    }
    let last = parts
        .last()
        .ok_or_else(|| anyhow::anyhow!("配置键不能为空"))?;
    current.insert((*last).to_string(), value);
    Ok(())
}

fn redact_secret(value: &str) -> String {
    if value.len() <= 8 {
        "***".into()
    } else {
        format!("{}...{}", &value[..4], &value[value.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("deepcoder_config_{name}_{}", std::process::id()));
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn config_default_values() {
        let dir = temp_dir("defaults");
        let config =
            Config::load_from_paths(dir.join("global.toml"), dir.join("project.toml")).unwrap();
        assert_eq!(config.provider.r#type, "deepseek");
        assert_eq!(config.provider.model, "deepseek-chat");
        assert_eq!(config.sandbox.mode, "auto");
        assert!(config.ui.show_reasoning);
    }

    #[test]
    fn config_loads_global_and_project_file() {
        let dir = temp_dir("layers");
        let global = dir.join("global.toml");
        let project = dir.join("project.toml");
        std::fs::write(
            &global,
            r#"
[provider]
model = "global-model"
[ui]
theme = "global"
"#,
        )
        .unwrap();
        std::fs::write(
            &project,
            r#"
[provider]
model = "project-model"
"#,
        )
        .unwrap();

        let config = Config::load_from_paths(global, project).unwrap();
        assert_eq!(config.provider.model, "project-model");
        assert_eq!(config.ui.theme, "global");
    }

    #[test]
    fn config_set_persists_global_file() {
        let dir = temp_dir("set");
        let path = dir.join("config.toml");
        Config::set_value_at_path(&path, "provider.model", "deepseek-reasoner").unwrap();
        Config::set_value_at_path(&path, "ui.show_reasoning", "false").unwrap();

        let config = Config::load_from_paths(path, dir.join("project.toml")).unwrap();
        assert_eq!(config.provider.model, "deepseek-reasoner");
        assert!(!config.ui.show_reasoning);
    }

    #[test]
    fn runtime_environment_overrides_data_directory() {
        let dir = temp_dir("runtime_data_dir");
        let mut config =
            Config::load_from_paths(dir.join("global.toml"), dir.join("project.toml")).unwrap();
        let runtime_dir = dir.join("persistent");

        config
            .apply_environment_overrides_with(|key| {
                Ok((key == "DEEPCODER_DATA_DIR").then(|| runtime_dir.display().to_string()))
            })
            .unwrap();

        assert_eq!(config.system.data_dir, runtime_dir);
    }

    #[test]
    fn invalid_key_rejected() {
        let dir = temp_dir("invalid");
        let error = Config::set_value_at_path(&dir.join("config.toml"), "unknown.key", "value")
            .unwrap_err();
        assert!(error.to_string().contains("未知配置键"));
    }

    #[test]
    fn config_list_redacts_secrets() {
        let mut config = Config::load_from_paths(
            temp_dir("redact").join("global.toml"),
            temp_dir("redact").join("project.toml"),
        )
        .unwrap();
        config.api_key = Some("sk-1234567890".into());
        let values = config.list_values_redacted();
        let api_key = values
            .iter()
            .find(|(key, _)| key == "api_key")
            .map(|(_, value)| value)
            .unwrap();
        assert_ne!(api_key, "sk-1234567890");
        assert!(api_key.contains("..."));
    }
}
