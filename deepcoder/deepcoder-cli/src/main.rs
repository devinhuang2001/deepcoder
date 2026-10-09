//! DeepCoder CLI 入口
//!
//! 支持四种运行模式：交互 TUI、批处理执行、MCP 服务器、AppServer

#[cfg(test)]
mod release;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "deepcoder",
    version,
    about = "专为 DeepSeek V4 优化的 AI 编码助手"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 模型名称
    #[arg(long, global = true)]
    model: Option<String>,

    /// API Key
    #[arg(long, global = true, env = "DEEPSEEK_API_KEY")]
    api_key: Option<String>,

    /// 配置文件路径
    #[arg(long, global = true)]
    config: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// 交互式 TUI 模式（默认）
    #[command(name = "ui")]
    Interactive,

    /// 批处理执行
    #[command(name = "exec")]
    Exec {
        /// 查询内容
        query: String,
    },

    /// MCP 服务器模式
    #[command(name = "mcp-server")]
    McpServer,

    /// AppServer 模式
    #[command(name = "app-server")]
    AppServer {
        /// WebSocket 绑定地址
        #[arg(long, default_value = "127.0.0.1:8080")]
        ws: Option<String>,
        /// 使用 stdio JSON-RPC transport
        #[arg(long)]
        stdio: bool,
    },

    /// 管理全局配置
    #[command(name = "config")]
    Config {
        #[command(subcommand)]
        action: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// 读取配置项
    Get {
        /// 配置键，例如 provider.model
        key: String,
    },
    /// 设置配置项
    Set {
        /// 配置键，例如 provider.model
        key: String,
        /// 配置值
        value: String,
    },
    /// 列出配置项（secret 会脱敏）
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    let config_path = cli.config.as_ref().map(PathBuf::from);

    // 加载配置
    let mut config_builder = if let Some(path) = &config_path {
        deepcoder_config::Config::load_from_paths(
            path.clone(),
            PathBuf::from(".deepcoder/config.toml"),
        )?
    } else {
        deepcoder_config::Config::load_default()?
    };

    apply_cli_overrides(&mut config_builder, cli.model, cli.api_key);

    let config = config_builder;

    match cli.command.unwrap_or(Commands::Interactive) {
        Commands::Interactive => {
            // 启动 TUI
            deepcoder_tui::run(config).await?;
        }
        Commands::Exec { query } => {
            // 批处理模式
            ensure_exec_api_key(&config)?;
            let result = deepcoder_engine::run_exec(&config, &query).await?;
            println!("{}", result);
        }
        Commands::McpServer => {
            // MCP 服务器模式
            deepcoder_mcp::run_server(config).await?;
        }
        Commands::AppServer { ws, stdio } => {
            // AppServer 模式
            if stdio {
                deepcoder_app_server::run_stdio(config).await?;
            } else {
                let addr = ws.unwrap_or_else(|| "127.0.0.1:8080".into());
                deepcoder_app_server::run(config, &addr).await?;
            }
        }
        Commands::Config { action } => {
            let write_path =
                config_path.unwrap_or_else(deepcoder_config::Config::global_config_path);
            match action {
                ConfigCommand::Get { key } => match config.get_value(&key) {
                    Some(value) => println!("{value}"),
                    None => anyhow::bail!("未知配置键: {key}"),
                },
                ConfigCommand::Set { key, value } => {
                    deepcoder_config::Config::set_value_at_path(&write_path, &key, &value)?;
                    println!("updated {}", write_path.display());
                }
                ConfigCommand::List => {
                    for (key, value) in config.list_values_redacted() {
                        println!("{key}={value}");
                    }
                }
            }
        }
    }

    Ok(())
}

fn apply_cli_overrides(
    config: &mut deepcoder_config::Config,
    model: Option<String>,
    api_key: Option<String>,
) {
    if let Some(model) = model {
        config.provider.model = model;
    }
    if let Some(key) = api_key {
        config.api_key = Some(key);
    }
}

fn ensure_exec_api_key(config: &deepcoder_config::Config) -> anyhow::Result<()> {
    let env_key = std::env::var("DEEPSEEK_API_KEY").ok();
    ensure_exec_api_key_with_env(config, env_key.as_deref())
}

fn ensure_exec_api_key_with_env(
    config: &deepcoder_config::Config,
    env_key: Option<&str>,
) -> anyhow::Result<()> {
    let has_key = config
        .api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        || env_key.filter(|value| !value.trim().is_empty()).is_some();
    if has_key {
        Ok(())
    } else {
        anyhow::bail!(
            "DEEPSEEK_API_KEY 未设置。请设置环境变量 DEEPSEEK_API_KEY，或使用 --api-key。"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_parses_default_mode() {
        let cli = Cli::try_parse_from(["deepcoder"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn cli_parses_exec() {
        let cli = Cli::try_parse_from(["deepcoder", "exec", "hello"]).unwrap();
        match cli.command.unwrap() {
            Commands::Exec { query } => assert_eq!(query, "hello"),
            _ => panic!("expected exec command"),
        }
    }

    #[test]
    fn cli_applies_model_and_api_key_overrides() {
        let mut config = deepcoder_config::Config::load_default().unwrap();
        apply_cli_overrides(
            &mut config,
            Some("deepseek-reasoner".into()),
            Some("sk-test".into()),
        );
        assert_eq!(config.provider.model, "deepseek-reasoner");
        assert_eq!(config.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn missing_api_key_error_is_actionable() {
        let mut config = deepcoder_config::Config::load_default().unwrap();
        config.api_key = None;
        let error = ensure_exec_api_key_with_env(&config, None).unwrap_err();
        assert!(error.to_string().contains("DEEPSEEK_API_KEY"));
    }

    #[test]
    fn invalid_subcommand_is_rejected() {
        let error = Cli::try_parse_from(["deepcoder", "unknown"]).unwrap_err();
        assert!(error.to_string().contains("unknown"));
    }
}
