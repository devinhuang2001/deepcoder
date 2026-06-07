//! DeepCoder 扩展系统
//!
//! 类型化的 ExtensionRegistry，支持 6 种贡献点。

use std::sync::Arc;

/// 扩展数据存储
pub type ExtensionData = Arc<std::collections::HashMap<String, serde_json::Value>>;

/// 扩展注册表构建器
pub struct ExtensionRegistryBuilder {
    tool_providers: Vec<Arc<dyn ToolProvider>>,
    prompt_contributors: Vec<Arc<dyn PromptContributor>>,
    turn_hooks: Vec<Arc<dyn TurnHook>>,
}

impl ExtensionRegistryBuilder {
    pub fn new() -> Self {
        Self {
            tool_providers: Vec::new(),
            prompt_contributors: Vec::new(),
            turn_hooks: Vec::new(),
        }
    }

    pub fn with_tool_provider(mut self, provider: Arc<dyn ToolProvider>) -> Self {
        self.tool_providers.push(provider);
        self
    }

    pub fn with_prompt_contributor(mut self, contributor: Arc<dyn PromptContributor>) -> Self {
        self.prompt_contributors.push(contributor);
        self
    }

    pub fn with_turn_hook(mut self, hook: Arc<dyn TurnHook>) -> Self {
        self.turn_hooks.push(hook);
        self
    }

    pub fn build(self) -> ExtensionRegistry {
        ExtensionRegistry {
            tool_providers: self.tool_providers,
            prompt_contributors: self.prompt_contributors,
            turn_hooks: self.turn_hooks,
        }
    }
}

/// 不可变的扩展注册表
pub struct ExtensionRegistry {
    tool_providers: Vec<Arc<dyn ToolProvider>>,
    prompt_contributors: Vec<Arc<dyn PromptContributor>>,
    turn_hooks: Vec<Arc<dyn TurnHook>>,
}

impl ExtensionRegistry {
    pub fn tool_providers(&self) -> &[Arc<dyn ToolProvider>] {
        &self.tool_providers
    }

    pub fn prompt_contributors(&self) -> &[Arc<dyn PromptContributor>] {
        &self.prompt_contributors
    }

    pub fn turn_hooks(&self) -> &[Arc<dyn TurnHook>] {
        &self.turn_hooks
    }

    pub fn empty() -> Arc<Self> {
        Arc::new(ExtensionRegistryBuilder::new().build())
    }
}

/// 工具提供者贡献点
pub trait ToolProvider: Send + Sync {
    fn register_tools(&self, router: &deepcoder_tools::ToolRouter);
}

/// Prompt 贡献点
pub trait PromptContributor: Send + Sync {
    fn contribute_prompt(&self) -> Vec<String>;
}

/// 回合生命周期钩子
pub trait TurnHook: Send + Sync {
    fn on_turn_start(&self) {}
    fn on_turn_end(&self) {}
}
