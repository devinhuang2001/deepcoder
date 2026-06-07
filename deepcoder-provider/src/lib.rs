//! DeepCoder AI Provider 抽象层
//!
//! 定义 ModelProvider trait 及 DeepSeek V4 实现。

pub mod deepseek;

use async_trait::async_trait;
use deepcoder_types::provider::*;
use deepcoder_error::DeepCoderResult;

/// AI 模型提供者抽象
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// 返回 Provider 信息
    fn info(&self) -> &ProviderInfo;

    /// 声明能力
    fn capabilities(&self) -> &ProviderCapabilities;

    /// 流式聊天补全
    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> DeepCoderResult<Box<dyn StreamReceiver>>;
}

/// 流式响应接收器
#[async_trait]
pub trait StreamReceiver: Send + Sync {
    /// 接收下一个事件。返回 None 表示流结束。
    async fn next_event(&mut self) -> Option<DeepCoderResult<StreamEvent>>;
}

/// 创建 DeepSeek Provider
pub fn create_deepseek_provider(api_key: String, model: String, base_url: String) -> impl ModelProvider {
    deepseek::DeepSeekProvider::new(api_key, model, base_url)
}
