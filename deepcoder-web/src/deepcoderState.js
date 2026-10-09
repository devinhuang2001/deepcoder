export const roleToBubbleRole = (role) => {
  if (role === 'User') return 'user';
  if (role === 'Assistant') return 'assistant';
  return 'system';
};

export const contentToText = (content) => {
  if (typeof content?.Text === 'string') return content.Text;
  return '';
};

export const messageToBubble = (message, fallbackIdFactory = defaultFallbackId) => {
  const content = (message.contents || [])
    .map(contentToText)
    .filter(Boolean)
    .join('\n\n')
    .trim();
  if (!content) return null;
  return {
    id: message.id || fallbackIdFactory(),
    role: roleToBubbleRole(message.role),
    content,
  };
};

export const extractDiagnostics = (messages = []) => {
  const reasoning = [];
  const tools = [];
  for (const message of messages) {
    for (const content of message.contents || []) {
      if (content?.Reasoning?.content) {
        reasoning.push(content.Reasoning.content);
      } else if (content?.ToolCall) {
        tools.push({
          id: content.ToolCall.id || `tool-call-${tools.length}`,
          label: content.ToolCall.name || 'unknown',
          detail: '请求执行',
          tone: 'blue',
        });
      } else if (content?.ToolResult) {
        tools.push({
          id: content.ToolResult.id || `tool-result-${tools.length}`,
          label: content.ToolResult.id || 'tool',
          detail: content.ToolResult.is_error ? '失败' : '完成',
          tone: content.ToolResult.is_error ? 'rose' : 'emerald',
        });
      }
    }
  }
  return {
    reasoningText: reasoning.join('\n\n'),
    toolEvents: tools.slice(-12),
  };
};

export const buildThreadTranscript = (thread, messages, now = Date.now()) => {
  const restored = (messages || []).map(messageToBubble).filter(Boolean);
  const systemMessage = {
    id: `system-thread-${thread.id}-${now}`,
    role: 'system',
    content: `已载入会话 ${thread.id.slice(0, 8)}`,
  };
  if (restored.length === 0) {
    return [
      systemMessage,
      { id: `assistant-empty-${thread.id}`, role: 'assistant', content: '这个会话还没有消息。' },
    ];
  }
  return [systemMessage, ...restored];
};

export const createClientTurnId = (scope = globalThis) => {
  return scope.crypto?.randomUUID?.()
    || `turn-${Date.now()}-${Math.random().toString(36).slice(2)}`;
};

export const resolveWebSocketUrl = (configuredUrl, location = globalThis.location) => {
  const explicitUrl = configuredUrl?.trim();
  if (explicitUrl) return explicitUrl;
  const protocol = location?.protocol === 'https:' ? 'wss:' : 'ws:';
  const host = location?.host?.trim();
  if (host) return `${protocol}//${host}/ws`;
  return 'ws://127.0.0.1:8080';
};

export const createWebSocketProtocols = (accessToken) => {
  const token = accessToken?.trim();
  if (!token) return ['deepcoder-v1'];
  if (token.length < 32 || token.length > 256 || !/^[A-Za-z0-9_-]+$/.test(token)) {
    throw new Error('访问令牌必须是 32-256 位 URL 安全字符');
  }
  return ['deepcoder-v1', `deepcoder-token.${token}`];
};

export const calculateReconnectDelay = (attempt, random = Math.random) => {
  const exponent = Math.max(0, Math.min(Number(attempt) || 0, 20));
  const baseDelay = Math.min(30_000, 1000 * (2 ** exponent));
  const jitter = Math.round(Math.max(0, Math.min(1, random())) * 250);
  return Math.min(30_250, baseDelay + jitter);
};

const defaultFallbackId = () => `message-${Date.now()}-${Math.random().toString(36).slice(2)}`;
