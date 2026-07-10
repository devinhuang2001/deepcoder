//! DeepCoder 扩展系统
//!
//! 类型化的 ExtensionRegistry，支持 6 种贡献点。

use std::sync::Arc;

/// 扩展数据存储
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtensionScope {
    Session,
    Thread,
    Turn,
}

#[derive(Debug, Clone, Default)]
pub struct ExtensionData {
    session: std::collections::HashMap<String, serde_json::Value>,
    thread: std::collections::HashMap<String, serde_json::Value>,
    turn: std::collections::HashMap<String, serde_json::Value>,
}

impl ExtensionData {
    pub fn insert(
        &mut self,
        scope: ExtensionScope,
        key: impl Into<String>,
        value: serde_json::Value,
    ) {
        self.scope_mut(scope).insert(key.into(), value);
    }

    pub fn get(&self, scope: ExtensionScope, key: &str) -> Option<&serde_json::Value> {
        self.scope(scope).get(key)
    }

    pub fn clear_scope(&mut self, scope: ExtensionScope) {
        self.scope_mut(scope).clear();
    }

    fn scope(
        &self,
        scope: ExtensionScope,
    ) -> &std::collections::HashMap<String, serde_json::Value> {
        match scope {
            ExtensionScope::Session => &self.session,
            ExtensionScope::Thread => &self.thread,
            ExtensionScope::Turn => &self.turn,
        }
    }

    fn scope_mut(
        &mut self,
        scope: ExtensionScope,
    ) -> &mut std::collections::HashMap<String, serde_json::Value> {
        match scope {
            ExtensionScope::Session => &mut self.session,
            ExtensionScope::Thread => &mut self.thread,
            ExtensionScope::Turn => &mut self.turn,
        }
    }
}

/// 扩展注册表构建器
pub struct ExtensionRegistryBuilder {
    tool_providers: Vec<Arc<dyn ToolProvider>>,
    prompt_contributors: Vec<Arc<dyn PromptContributor>>,
    turn_hooks: Vec<Arc<dyn TurnHook>>,
    event_listeners: Vec<Arc<dyn EventListener>>,
    ui_contributors: Vec<Arc<dyn UiContributor>>,
    persistence_contributors: Vec<Arc<dyn PersistenceContributor>>,
}

impl ExtensionRegistryBuilder {
    pub fn new() -> Self {
        Self {
            tool_providers: Vec::new(),
            prompt_contributors: Vec::new(),
            turn_hooks: Vec::new(),
            event_listeners: Vec::new(),
            ui_contributors: Vec::new(),
            persistence_contributors: Vec::new(),
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

    pub fn with_event_listener(mut self, listener: Arc<dyn EventListener>) -> Self {
        self.event_listeners.push(listener);
        self
    }

    pub fn with_ui_contributor(mut self, contributor: Arc<dyn UiContributor>) -> Self {
        self.ui_contributors.push(contributor);
        self
    }

    pub fn with_persistence_contributor(
        mut self,
        contributor: Arc<dyn PersistenceContributor>,
    ) -> Self {
        self.persistence_contributors.push(contributor);
        self
    }

    pub fn build(self) -> ExtensionRegistry {
        ExtensionRegistry {
            tool_providers: self.tool_providers,
            prompt_contributors: self.prompt_contributors,
            turn_hooks: self.turn_hooks,
            event_listeners: self.event_listeners,
            ui_contributors: self.ui_contributors,
            persistence_contributors: self.persistence_contributors,
        }
    }
}

impl Default for ExtensionRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 不可变的扩展注册表
pub struct ExtensionRegistry {
    tool_providers: Vec<Arc<dyn ToolProvider>>,
    prompt_contributors: Vec<Arc<dyn PromptContributor>>,
    turn_hooks: Vec<Arc<dyn TurnHook>>,
    event_listeners: Vec<Arc<dyn EventListener>>,
    ui_contributors: Vec<Arc<dyn UiContributor>>,
    persistence_contributors: Vec<Arc<dyn PersistenceContributor>>,
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

    pub fn event_listeners(&self) -> &[Arc<dyn EventListener>] {
        &self.event_listeners
    }

    pub fn ui_contributors(&self) -> &[Arc<dyn UiContributor>] {
        &self.ui_contributors
    }

    pub fn persistence_contributors(&self) -> &[Arc<dyn PersistenceContributor>] {
        &self.persistence_contributors
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
#[async_trait::async_trait]
pub trait TurnHook: Send + Sync {
    async fn on_turn_start(&self) {}
    async fn on_turn_end(&self) {}
}

/// 事件监听贡献点
pub trait EventListener: Send + Sync {
    fn on_event(&self, event: &deepcoder_types::event::EngineEvent);
}

/// UI 贡献点
pub trait UiContributor: Send + Sync {
    fn status_segments(&self) -> Vec<String>;
}

/// 持久化贡献点
pub trait PersistenceContributor: Send + Sync {
    fn enrich_event(&self, event: serde_json::Value) -> serde_json::Value {
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockToolProvider;
    impl ToolProvider for MockToolProvider {
        fn register_tools(&self, _router: &deepcoder_tools::ToolRouter) {}
    }

    struct MockPromptContributor;
    impl PromptContributor for MockPromptContributor {
        fn contribute_prompt(&self) -> Vec<String> {
            vec!["prompt".into()]
        }
    }

    struct MockHook {
        called: AtomicBool,
    }

    #[async_trait::async_trait]
    impl TurnHook for MockHook {
        async fn on_turn_start(&self) {
            self.called.store(true, Ordering::SeqCst);
        }
    }

    struct MockEventListener;
    impl EventListener for MockEventListener {
        fn on_event(&self, _event: &deepcoder_types::event::EngineEvent) {}
    }

    struct MockUiContributor;
    impl UiContributor for MockUiContributor {
        fn status_segments(&self) -> Vec<String> {
            vec!["ready".into()]
        }
    }

    struct MockPersistenceContributor;
    impl PersistenceContributor for MockPersistenceContributor {}

    #[test]
    fn registry_builder_empty() {
        let registry = ExtensionRegistryBuilder::new().build();
        assert!(registry.tool_providers().is_empty());
        assert!(registry.prompt_contributors().is_empty());
        assert!(registry.turn_hooks().is_empty());
        assert!(registry.event_listeners().is_empty());
        assert!(registry.ui_contributors().is_empty());
        assert!(registry.persistence_contributors().is_empty());
    }

    #[test]
    fn registry_builder_adds_contributors() {
        let registry = ExtensionRegistryBuilder::new()
            .with_tool_provider(Arc::new(MockToolProvider))
            .with_prompt_contributor(Arc::new(MockPromptContributor))
            .with_turn_hook(Arc::new(MockHook {
                called: AtomicBool::new(false),
            }))
            .with_event_listener(Arc::new(MockEventListener))
            .with_ui_contributor(Arc::new(MockUiContributor))
            .with_persistence_contributor(Arc::new(MockPersistenceContributor))
            .build();

        assert_eq!(registry.tool_providers().len(), 1);
        assert_eq!(registry.prompt_contributors().len(), 1);
        assert_eq!(registry.turn_hooks().len(), 1);
        assert_eq!(registry.event_listeners().len(), 1);
        assert_eq!(registry.ui_contributors().len(), 1);
        assert_eq!(registry.persistence_contributors().len(), 1);
    }

    #[tokio::test]
    async fn turn_hooks_are_async_object_safe() {
        let hook = Arc::new(MockHook {
            called: AtomicBool::new(false),
        });
        let dyn_hook: Arc<dyn TurnHook> = hook.clone();
        dyn_hook.on_turn_start().await;
        assert!(hook.called.load(Ordering::SeqCst));
    }

    #[test]
    fn extension_data_scopes_are_isolated() {
        let mut data = ExtensionData::default();
        data.insert(ExtensionScope::Session, "key", serde_json::json!("session"));
        data.insert(ExtensionScope::Turn, "key", serde_json::json!("turn"));
        assert_eq!(data.get(ExtensionScope::Session, "key").unwrap(), "session");
        assert_eq!(data.get(ExtensionScope::Turn, "key").unwrap(), "turn");
        data.clear_scope(ExtensionScope::Turn);
        assert!(data.get(ExtensionScope::Turn, "key").is_none());
        assert!(data.get(ExtensionScope::Session, "key").is_some());
    }
}
