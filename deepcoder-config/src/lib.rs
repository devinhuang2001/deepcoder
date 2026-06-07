//! DeepCoder 配置系统
//!
//! TOML 分层配置：系统默认 → 全局配置 → 项目配置 → CLI 覆盖

use std::path::PathBuf;
use anyhow::{Context, Result};

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
        Self { mode: "auto".into() }
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
        Self { theme: "default".into(), show_reasoning: true }
    }
}

/// 系统配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemConfig {
    pub data_dir: PathBuf,
    pub max_tool_iterations: u32,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("deepcoder"),
            max_tool_iterations: 25,
        }
    }
}

impl Config {
    /// 加载配置（默认值 + 全局配置 + 项目配置 + 环境变量）
    pub fn load_default() -> Result<Self> {
        let mut config = Config {
            provider: ProviderConfig::default(),
            sandbox: SandboxConfig::default(),
            ui: UiConfig::default(),
            system: SystemConfig::default(),
            api_key: std::env::var("DEEPSEEK_API_KEY").ok(),
        };

        // 尝试加载全局配置
        let global_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("deepcoder")
            .join("config.toml");

        if global_path.exists() {
            let content = std::fs::read_to_string(&global_path)
                .context(format!("读取配置失败: {}", global_path.display()))?;
            let file_config: ConfigFile = toml::from_str(&content)
                .context(format!("解析配置失败: {}", global_path.display()))?;
            config.apply_file(file_config);
        }

        // 尝试加载项目配置
        let project_path = PathBuf::from(".deepcoder/config.toml");
        if project_path.exists() {
            let content = std::fs::read_to_string(&project_path)?;
            let file_config: ConfigFile = toml::from_str(&content)?;
            config.apply_file(file_config);
        }

        Ok(config)
    }

    fn apply_file(&mut self, file: ConfigFile) {
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
    }
}

/// 配置文件的序列化结构
#[derive(Debug, serde::Deserialize)]
struct ConfigFile {
    provider: Option<ProviderFileConfig>,
    sandbox: Option<SandboxFileConfig>,
    ui: Option<UiFileConfig>,
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
