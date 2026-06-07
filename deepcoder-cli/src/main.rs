//! DeepCoder CLI 入口
//!
//! 支持四种运行模式：交互 TUI、批处理执行、MCP 服务器、AppServer

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "deepcoder", version, about = "专为 DeepSeek V4 优化的 AI 编码助手")]
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

#[derive(Subcommand)]
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
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    // 加载配置
    let mut config_builder = deepcoder_config::Config::load_default()?;

    // CLI 覆盖
    if let Some(model) = cli.model {
        config_builder.provider.model = model;
    }
    if let Some(key) = cli.api_key {
        config_builder.api_key = Some(key);
    }

    let config = config_builder;

    match cli.command.unwrap_or(Commands::Interactive) {
        Commands::Interactive => {
            // 启动 TUI
            deepcoder_tui::run(config).await?;
        }
        Commands::Exec { query } => {
            // 批处理模式
            let result = deepcoder_engine::run_exec(&config, &query).await?;
            println!("{}", result);
        }
        Commands::McpServer => {
            // MCP 服务器模式
            deepcoder_mcp::run_server(config).await?;
        }
        Commands::AppServer { ws } => {
            // AppServer 模式
            let addr = ws.unwrap_or_else(|| "127.0.0.1:8080".into());
            deepcoder_app_server::run(config, &addr).await?;
        }
    }

    Ok(())
}
