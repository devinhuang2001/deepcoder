import { useState, useRef, useEffect, useCallback } from 'react';
import { Send, Bot, User, Command, Square, Plus, Brain, Wrench, Gauge, MessageSquare } from 'lucide-react';
import gsap from 'gsap';
import { useGSAP } from '@gsap/react';
import {
  buildThreadTranscript,
  calculateReconnectDelay,
  createClientTurnId,
  createWebSocketProtocols,
  extractDiagnostics,
  resolveWebSocketUrl,
} from './deepcoderState.js';

gsap.registerPlugin(useGSAP);

const statusTone = (status, busy, cancelling) => {
  if (cancelling) return 'bg-amber-500 text-black border-amber-300';
  if (busy) return 'bg-lime-400 text-black border-lime-200';
  if (status === '已连接 AppServer') return 'bg-emerald-400 text-black border-emerald-200';
  if (status === '连接错误' || status === '未连接') return 'bg-red-500 text-white border-red-300';
  return 'bg-stone-500 text-white border-stone-300';
};

const toolTone = (tone) => {
  if (tone === 'rose') return 'border-red-400/25 bg-red-950/25 text-red-200';
  if (tone === 'emerald') return 'border-emerald-400/25 bg-emerald-950/25 text-emerald-200';
  return 'border-cyan-400/25 bg-cyan-950/25 text-cyan-200';
};

const productionAuthRequired = import.meta.env.VITE_DEEPCODER_REQUIRE_AUTH === 'true';

export default function App() {
  const [messages, setMessages] = useState([
    { id: 'system-ready', role: 'system', content: '正在连接 DeepCoder AppServer...' },
    { id: 'welcome', role: 'assistant', content: '你好，我是 DeepCoder。连接后你可以直接把任务发给 Rust 引擎。' },
  ]);
  const [input, setInput] = useState('');
  const [connectionStatus, setConnectionStatus] = useState('连接中');
  const [engineStatus, setEngineStatus] = useState('等待 AppServer');
  const [threadId, setThreadId] = useState(null);
  const [threads, setThreads] = useState([]);
  const [reasoningText, setReasoningText] = useState('');
  const [toolEvents, setToolEvents] = useState([]);
  const [tokenUsage, setTokenUsage] = useState(null);
  const [isBusy, setIsBusy] = useState(false);
  const [isCancelling, setIsCancelling] = useState(false);
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [accessTokenInput, setAccessTokenInput] = useState('');
  const [authError, setAuthError] = useState('');
  const containerRef = useRef(null);
  const messagesEndRef = useRef(null);
  const wsRef = useRef(null);
  const pendingRef = useRef(new Map());
  const requestIdRef = useRef(1);
  const activeAssistantIdRef = useRef(null);
  const activeTurnIdRef = useRef(null);
  const reasoningRef = useRef('');
  const toolsRef = useRef([]);
  const accessTokenRef = useRef('');
  const reconnectTimerRef = useRef(null);
  const reconnectAttemptRef = useRef(0);
  const mountedRef = useRef(false);

  const requestJsonRpc = useCallback((method, params = {}) => {
    const socket = wsRef.current;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error('AppServer WebSocket 尚未连接'));
    }
    const id = requestIdRef.current++;
    const payload = { jsonrpc: '2.0', id, method, params };
    return new Promise((resolve, reject) => {
      pendingRef.current.set(id, { resolve, reject });
      socket.send(JSON.stringify(payload));
    });
  }, []);

  const refreshThreads = useCallback(async () => {
    const result = await requestJsonRpc('thread/list');
    const list = Array.isArray(result.threads) ? result.threads : [];
    setThreads(list);
    return list;
  }, [requestJsonRpc]);

  const loadThread = useCallback(async (targetThread) => {
    const result = await requestJsonRpc('thread/get', { thread_id: targetThread.id });
    const diagnostics = extractDiagnostics(result.messages);
    setThreadId(result.thread.id);
    setMessages(buildThreadTranscript(result.thread, result.messages));
    setReasoningText(diagnostics.reasoningText);
    setToolEvents(diagnostics.toolEvents);
    setTokenUsage(null);
    return result.thread;
  }, [requestJsonRpc]);

  const connectWebSocket = useCallback((accessToken = accessTokenRef.current) => {
    if (!mountedRef.current) return;
    if (reconnectTimerRef.current) {
      window.clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    const existingSocket = wsRef.current;
    if (existingSocket) {
      existingSocket.onopen = null;
      existingSocket.onmessage = null;
      existingSocket.onerror = null;
      existingSocket.onclose = null;
      existingSocket.close();
    }

    let protocols;
    try {
      protocols = createWebSocketProtocols(accessToken);
    } catch (error) {
      setAuthError(error.message);
      setIsAuthenticating(false);
      return;
    }

    accessTokenRef.current = accessToken?.trim() || '';
    const wsUrl = resolveWebSocketUrl(import.meta.env.VITE_DEEPCODER_WS_URL, window.location);
    const socket = new WebSocket(wsUrl, protocols);
    wsRef.current = socket;
    setConnectionStatus('连接中');
    setEngineStatus(productionAuthRequired ? '认证中' : '等待 AppServer');
    let opened = false;

    socket.onopen = async () => {
      if (wsRef.current !== socket) return;
      opened = true;
      reconnectAttemptRef.current = 0;
      setIsAuthenticated(true);
      setIsAuthenticating(false);
      setAccessTokenInput('');
      setAuthError('');
      setConnectionStatus('已连接 AppServer');
      setEngineStatus('初始化中');
      try {
        await requestJsonRpc('initialize');
        const list = await refreshThreads();
        let activeThread = list[0];
        if (!activeThread) {
          const result = await requestJsonRpc('thread/create');
          activeThread = result.thread;
          setThreads([activeThread]);
        }
        await loadThread(activeThread);
        setEngineStatus('Ready');
      } catch (error) {
        setEngineStatus('连接失败');
        setMessages(prev => [...prev, { id: `error-${Date.now()}`, role: 'system', content: error.message }]);
      }
    };

    socket.onmessage = (event) => {
      if (wsRef.current !== socket) return;
      let response;
      try {
        response = JSON.parse(event.data);
      } catch (error) {
        setMessages(prev => [...prev, { id: `error-${Date.now()}`, role: 'system', content: `响应解析失败: ${error.message}` }]);
        return;
      }
      if (response.method === 'turn/event') {
        const event = response.params || {};
        if (event.type === 'turn_cancelled') {
          if (event.turn_id && activeTurnIdRef.current && event.turn_id !== activeTurnIdRef.current) {
            return;
          }
          const assistantId = activeAssistantIdRef.current;
          if (assistantId) {
            setMessages(prev => prev.map(msg => (
              msg.id === assistantId ? { ...msg, content: '已停止本轮请求。' } : msg
            )));
          }
          activeAssistantIdRef.current = null;
          activeTurnIdRef.current = null;
          setIsBusy(false);
          setIsCancelling(false);
          setToolEvents(prev => [
            { id: `cancel-${Date.now()}`, label: event.turn_id || 'turn', detail: '已停止', tone: 'rose' },
            ...prev,
          ].slice(0, 12));
          setEngineStatus('Cancelled');
          return;
        }
        const assistantId = activeAssistantIdRef.current;
        if (!assistantId) return;
        if (event.type === 'text_delta') {
          setMessages(prev => prev.map(msg => {
            if (msg.id !== assistantId) return msg;
            const current = msg.content === '思考中...' ? '' : msg.content;
            return { ...msg, content: `${current}${event.content || ''}` };
          }));
        } else if (event.type === 'reasoning_delta') {
          reasoningRef.current += event.content || '';
          setReasoningText(prev => `${prev}${event.content || ''}`);
          setEngineStatus('Reasoning');
        } else if (event.type === 'tool_call') {
          const label = event.tool_call?.tool_name || event.tool_call?.name || 'unknown';
          const item = { id: event.tool_call?.call_id || event.tool_call?.id || `tool-${Date.now()}`, label, detail: '请求执行', tone: 'blue' };
          toolsRef.current.push(`工具请求: ${label}`);
          setToolEvents(prev => [item, ...prev].slice(0, 12));
          setEngineStatus('Tool call');
        } else if (event.type === 'tool_result') {
          const detail = event.is_error ? '失败' : '完成';
          toolsRef.current.push(`工具结果: ${detail} ${event.tool_call_id || ''}`);
          setToolEvents(prev => [
            { id: event.tool_call_id || `tool-result-${Date.now()}`, label: event.tool_call_id || 'tool', detail, tone: event.is_error ? 'rose' : 'emerald' },
            ...prev,
          ].slice(0, 12));
        } else if (event.type === 'turn_complete') {
          setTokenUsage(event.token_usage || null);
          setEngineStatus('Finalizing');
        }
        return;
      }

      const pending = pendingRef.current.get(response.id);
      if (!pending) return;
      pendingRef.current.delete(response.id);
      if (response.error) {
        pending.reject(new Error(response.error.message || 'AppServer 返回错误'));
      } else {
        pending.resolve(response.result);
      }
    };

    socket.onclose = () => {
      if (wsRef.current !== socket) return;
      wsRef.current = null;
      setConnectionStatus('未连接');
      setEngineStatus('AppServer 已断开');
      setIsAuthenticating(false);
      setIsBusy(false);
      setIsCancelling(false);
      setReasoningText('');
      setToolEvents([]);
      setTokenUsage(null);
      activeAssistantIdRef.current = null;
      activeTurnIdRef.current = null;
      pendingRef.current.forEach(({ reject }) => reject(new Error('AppServer WebSocket 已断开')));
      pendingRef.current.clear();
      if (!mountedRef.current) return;
      if (productionAuthRequired && !opened) {
        accessTokenRef.current = '';
        setIsAuthenticated(false);
        setAuthError('认证失败：请检查访问令牌后重试。');
        setEngineStatus('认证失败');
        return;
      }
      const attempt = reconnectAttemptRef.current;
      reconnectAttemptRef.current += 1;
      const delay = calculateReconnectDelay(attempt);
      setEngineStatus(`将在 ${Math.ceil(delay / 1000)} 秒后重连`);
      reconnectTimerRef.current = window.setTimeout(() => {
        reconnectTimerRef.current = null;
        connectWebSocket(accessTokenRef.current);
      }, delay);
    };

    socket.onerror = () => {
      if (wsRef.current !== socket) return;
      setConnectionStatus('连接错误');
      setEngineStatus(productionAuthRequired ? '认证或网络错误' : '请先启动 deepcoder app-server');
    };
  }, [loadThread, refreshThreads, requestJsonRpc]);

  useEffect(() => {
    mountedRef.current = true;
    if (productionAuthRequired) {
      setConnectionStatus('需要认证');
      setEngineStatus('请输入访问令牌');
    } else {
      connectWebSocket('');
    }
    return () => {
      mountedRef.current = false;
      accessTokenRef.current = '';
      if (reconnectTimerRef.current) {
        window.clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      const socket = wsRef.current;
      wsRef.current = null;
      if (socket) {
        socket.onopen = null;
        socket.onmessage = null;
        socket.onerror = null;
        socket.onclose = null;
        socket.close();
      }
    };
  }, [connectWebSocket]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useGSAP(() => {
    gsap.from('.msg-bubble', {
      y: 14,
      opacity: 0,
      duration: 0.34,
      stagger: 0.025,
      ease: 'power2.out',
      clearProps: 'all',
    });
  }, { scope: containerRef, dependencies: [messages.length] });

  const getOrCreateThread = async () => {
    if (threadId) return threadId;
    const result = await requestJsonRpc('thread/create');
    setThreadId(result.thread.id);
    setThreads(prev => [result.thread, ...prev.filter(thread => thread.id !== result.thread.id)]);
    return result.thread.id;
  };

  const handleNewThread = async () => {
    if (isBusy || connectionStatus !== '已连接 AppServer') return;
    setEngineStatus('Creating thread');
    try {
      const result = await requestJsonRpc('thread/create');
      setThreadId(result.thread.id);
      setThreads(prev => [result.thread, ...prev.filter(thread => thread.id !== result.thread.id)]);
      setReasoningText('');
      setToolEvents([]);
      setTokenUsage(null);
      setMessages([
        { id: `system-${Date.now()}`, role: 'system', content: `已创建会话 ${result.thread.id.slice(0, 8)}` },
        { id: `assistant-${Date.now()}`, role: 'assistant', content: '新会话已准备好。' },
      ]);
      setEngineStatus('Ready');
    } catch (error) {
      setEngineStatus('Error');
      setMessages(prev => [...prev, { id: `error-${Date.now()}`, role: 'system', content: `创建会话失败: ${error.message}` }]);
    }
  };

  const handleSelectThread = async (thread) => {
    if (isBusy || thread.id === threadId) return;
    setEngineStatus('Loading thread');
    try {
      await loadThread(thread);
      setEngineStatus('Ready');
    } catch (error) {
      setEngineStatus('Error');
      setMessages(prev => [...prev, { id: `error-${Date.now()}`, role: 'system', content: `载入会话失败: ${error.message}` }]);
    }
  };

  const renderTurnEvents = (events) => {
    let text = '';
    for (const event of events || []) {
      if (event.type === 'text_delta') {
        text += event.content || '';
      } else if (event.type === 'error') {
        text += `\n错误: ${event.message}`;
      }
    }
    return text || '本轮没有返回文本内容。';
  };

  const handleSend = async (e) => {
    e.preventDefault();
    const query = input.trim();
    if (!query || isBusy) return;

    const assistantId = `assistant-${Date.now()}`;
    const turnId = createClientTurnId();
    activeAssistantIdRef.current = assistantId;
    activeTurnIdRef.current = turnId;
    reasoningRef.current = '';
    toolsRef.current = [];
    setReasoningText('');
    setToolEvents([]);
    setTokenUsage(null);
    const newMsg = { id: `user-${Date.now()}`, role: 'user', content: query };
    setMessages(prev => [...prev, newMsg, { id: assistantId, role: 'assistant', content: '思考中...' }]);
    setInput('');
    setIsBusy(true);
    setEngineStatus('Running');

    try {
      const activeThreadId = await getOrCreateThread();
      const result = await requestJsonRpc('turn/start', {
        thread_id: activeThreadId,
        input: query,
        turn_id: turnId,
      });
      const content = renderTurnEvents(result.events);
      setMessages(prev => prev.map(msg => (
        msg.id === assistantId ? { ...msg, content } : msg
      )));
      setTokenUsage(result.token_usage || null);
      refreshThreads().catch(() => {});
      setEngineStatus('Ready');
    } catch (error) {
      const wasCancelled = error.message.toLowerCase().includes('cancelled')
        || error.message.includes('取消');
      setMessages(prev => prev.map(msg => (
        msg.id === assistantId
          ? { ...msg, role: wasCancelled ? 'assistant' : 'system', content: wasCancelled ? '已停止本轮请求。' : `请求失败: ${error.message}` }
          : msg
      )));
      setEngineStatus(wasCancelled ? 'Cancelled' : 'Error');
    } finally {
      if (activeAssistantIdRef.current === assistantId) {
        activeAssistantIdRef.current = null;
      }
      if (activeTurnIdRef.current === turnId) {
        activeTurnIdRef.current = null;
      }
      setIsBusy(false);
      setIsCancelling(false);
    }
  };

  const handleStop = async () => {
    const turnId = activeTurnIdRef.current;
    if (!turnId || isCancelling) return;
    setIsCancelling(true);
    setEngineStatus('Cancelling');
    try {
      await requestJsonRpc('turn/cancel', { turn_id: turnId });
    } catch (error) {
      setMessages(prev => [...prev, { id: `error-${Date.now()}`, role: 'system', content: `停止失败: ${error.message}` }]);
      setIsCancelling(false);
      setEngineStatus('Error');
    }
  };

  const handleAuthenticate = (event) => {
    event.preventDefault();
    const token = accessTokenInput.trim();
    setAuthError('');
    setIsAuthenticating(true);
    connectWebSocket(token);
  };

  const activeThread = threads.find(thread => thread.id === threadId);
  const shortThreadId = threadId ? threadId.slice(0, 8) : 'no-thread';
  const statusClass = statusTone(connectionStatus, isBusy, isCancelling);
  const messageCount = activeThread?.message_count ?? messages.filter(message => message.role !== 'system').length;

  return (
    <div className="h-screen overflow-hidden bg-[#0d0f0c] text-stone-200 selection:bg-lime-400/25">
      <div className="pointer-events-none fixed inset-0 opacity-45 [background-image:linear-gradient(rgba(255,255,255,0.035)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.03)_1px,transparent_1px)] [background-size:34px_34px]" />
      {productionAuthRequired && !isAuthenticated && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#090b08]/92 px-4 backdrop-blur-sm" role="dialog" aria-modal="true" aria-labelledby="auth-title">
          <form onSubmit={handleAuthenticate} className="w-full max-w-md rounded-2xl border border-[#343a31] bg-[#11140f] p-6 shadow-[0_28px_80px_rgba(0,0,0,0.55)] md:p-8">
            <div className="flex h-11 w-11 items-center justify-center rounded-xl border border-lime-300/25 bg-lime-300/10 text-lime-200">
              <Command className="h-5 w-5" />
            </div>
            <p className="mt-5 text-xs font-semibold uppercase tracking-[0.2em] text-lime-200/70">Secure workspace</p>
            <h1 id="auth-title" className="mt-2 text-2xl font-semibold text-stone-100">连接 DeepCoder</h1>
            <p className="mt-3 text-sm leading-6 text-stone-400">
              输入部署时配置的访问令牌。令牌仅保存在当前页面内存中，刷新或关闭页面后会清除。
            </p>
            <label htmlFor="access-token" className="mt-6 block text-xs font-medium uppercase tracking-[0.14em] text-stone-500">
              Access token
            </label>
            <input
              id="access-token"
              type="password"
              value={accessTokenInput}
              onChange={(event) => setAccessTokenInput(event.target.value)}
              minLength={32}
              maxLength={256}
              autoComplete="off"
              spellCheck="false"
              autoFocus
              placeholder="32 位以上 URL 安全令牌"
              className="mt-2 h-12 w-full rounded-lg border border-[#343a31] bg-[#0d100c] px-4 text-sm text-stone-100 outline-none transition placeholder:text-stone-700 focus:border-lime-300/50 focus:ring-2 focus:ring-lime-300/10"
            />
            {authError && (
              <p className="mt-3 rounded-lg border border-red-400/20 bg-red-950/25 px-3 py-2 text-sm text-red-200" role="alert">
                {authError}
              </p>
            )}
            <button
              type="submit"
              disabled={accessTokenInput.trim().length < 32 || isAuthenticating}
              className="mt-5 flex h-11 w-full items-center justify-center rounded-lg border border-lime-200/40 bg-lime-300 text-sm font-semibold text-black transition hover:bg-lime-200 disabled:cursor-not-allowed disabled:border-[#30362d] disabled:bg-[#20251d] disabled:text-stone-600"
            >
              {isAuthenticating ? '正在认证…' : '安全连接'}
            </button>
          </form>
        </div>
      )}
      <div className="relative flex h-full">
        <aside className="hidden w-72 shrink-0 border-r border-[#2a2f28] bg-[#11140f]/95 lg:flex lg:flex-col">
          <div className="border-b border-[#2a2f28] px-4 py-4">
            <div className="flex items-center gap-3">
              <div className="flex h-9 w-9 items-center justify-center rounded-lg border border-lime-300/20 bg-lime-300/10">
                <Command className="h-4 w-4 text-lime-200" />
              </div>
              <div className="min-w-0">
                <p className="text-sm font-semibold tracking-wide text-stone-100">DeepCoder</p>
                <p className="truncate text-xs text-stone-500">{shortThreadId}</p>
              </div>
            </div>
            <button
              type="button"
              onClick={handleNewThread}
              disabled={isBusy || connectionStatus !== '已连接 AppServer'}
              className="mt-4 flex h-9 w-full items-center justify-center gap-2 rounded-lg border border-[#3a4037] bg-[#1a1f17] text-sm font-medium text-stone-100 transition hover:border-lime-300/40 hover:bg-[#212719] disabled:cursor-not-allowed disabled:opacity-45"
            >
              <Plus className="h-4 w-4" />
              新建会话
            </button>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto px-3 py-3 no-scrollbar">
            <div className="mb-2 flex items-center justify-between px-1 text-[11px] font-semibold uppercase tracking-[0.16em] text-stone-500">
              <span>Sessions</span>
              <span>{threads.length}</span>
            </div>
            <div className="space-y-1.5">
              {threads.length === 0 ? (
                <div className="rounded-lg border border-dashed border-[#33382f] px-3 py-4 text-sm text-stone-500">
                  暂无会话
                </div>
              ) : threads.map(thread => {
                const active = thread.id === threadId;
                return (
                  <button
                    key={thread.id}
                    type="button"
                    onClick={() => handleSelectThread(thread)}
                    disabled={isBusy}
                    className={`w-full rounded-lg border px-3 py-3 text-left transition disabled:cursor-not-allowed disabled:opacity-55 ${
                      active
                        ? 'border-lime-300/35 bg-lime-300/[0.08] text-stone-100'
                        : 'border-transparent text-stone-400 hover:border-[#353b33] hover:bg-[#171b14]'
                    }`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <p className="truncate text-sm font-medium">{thread.model || 'DeepCoder 会话'}</p>
                      <span className={`h-2 w-2 rounded-full ${active ? 'bg-lime-300' : 'bg-stone-700'}`} />
                    </div>
                    <div className="mt-1 flex items-center justify-between gap-2 text-xs text-stone-500">
                      <span className="truncate">{thread.id.slice(0, 8)}</span>
                      <span>{thread.message_count || 0} msgs</span>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          <div className="border-t border-[#2a2f28] px-4 py-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${connectionStatus === '已连接 AppServer' ? 'bg-emerald-400' : 'bg-red-400'}`} />
                <span className="truncate text-xs text-stone-400">{connectionStatus}</span>
              </div>
              <span className="rounded-md border border-[#30362d] px-2 py-1 text-[10px] uppercase tracking-wide text-stone-500">{productionAuthRequired ? 'secure' : 'local'}</span>
            </div>
          </div>
        </aside>

        <main className="flex min-w-0 flex-1 flex-col" ref={containerRef}>
          <header className="flex h-16 shrink-0 items-center justify-between border-b border-[#2a2f28] bg-[#0f120e]/92 px-4 md:px-6">
            <div className="flex min-w-0 items-center gap-3">
              <div className="flex h-9 w-9 items-center justify-center rounded-lg border border-[#353b33] bg-[#171b14]">
                <Bot className="h-4 w-4 text-lime-200" />
              </div>
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <h1 className="truncate text-sm font-semibold tracking-wide text-stone-100">Workspace</h1>
                  <span className="hidden rounded-md border border-[#30362d] px-2 py-0.5 text-[10px] uppercase tracking-wide text-stone-500 sm:inline">
                    {activeThread?.model || 'deepseek'}
                  </span>
                </div>
                <p className="truncate text-xs text-stone-500">{messageCount} messages · {shortThreadId}</p>
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <button
                type="button"
                onClick={handleNewThread}
                disabled={isBusy || connectionStatus !== '已连接 AppServer'}
                title="新建会话"
                className="flex h-9 w-9 items-center justify-center rounded-lg border border-[#343a31] bg-[#151913] text-stone-300 transition hover:border-lime-300/35 hover:text-lime-100 disabled:cursor-not-allowed disabled:opacity-45 lg:hidden"
              >
                <Plus className="h-4 w-4" />
              </button>
              <div className={`max-w-[8.5rem] truncate rounded-lg border px-3 py-1.5 text-xs font-semibold md:max-w-none ${statusClass}`}>
                {engineStatus}
              </div>
            </div>
          </header>

          <section className="min-h-0 flex-1 overflow-y-auto px-3 py-5 no-scrollbar md:px-8">
            <div className="mx-auto flex max-w-4xl flex-col gap-4">
              {messages.map((msg) => (
                <div key={msg.id} className={`msg-bubble flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
                  <div className={`flex max-w-[min(780px,92%)] gap-3 ${msg.role === 'user' ? 'flex-row-reverse' : 'flex-row'}`}>
                    <div className={`mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border ${
                      msg.role === 'user'
                        ? 'border-cyan-300/25 bg-cyan-300/10 text-cyan-100'
                        : msg.role === 'system'
                          ? 'border-amber-300/20 bg-amber-300/10 text-amber-100'
                          : 'border-lime-300/20 bg-lime-300/10 text-lime-100'
                    }`}>
                      {msg.role === 'user' ? <User className="h-4 w-4" /> : msg.role === 'system' ? <Command className="h-4 w-4" /> : <Bot className="h-4 w-4" />}
                    </div>
                    <div className={`rounded-lg border px-4 py-3 text-[15px] leading-7 shadow-sm ${
                      msg.role === 'user'
                        ? 'border-cyan-300/20 bg-cyan-950/25 text-cyan-50'
                        : msg.role === 'system'
                          ? 'border-[#3b3528] bg-[#18150e] text-amber-100/85'
                          : 'border-[#30362d] bg-[#151913] text-stone-100'
                    }`}>
                      <div className="whitespace-pre-wrap break-words">{msg.content}</div>
                    </div>
                  </div>
                </div>
              ))}
              <div ref={messagesEndRef} />
            </div>
          </section>

          <footer className="shrink-0 border-t border-[#2a2f28] bg-[#0f120e]/95 px-3 py-3 md:px-8 md:py-4">
            <form onSubmit={handleSend} className="mx-auto flex max-w-4xl items-center gap-2 rounded-lg border border-[#343a31] bg-[#151913] p-2 shadow-[0_14px_34px_rgba(0,0,0,0.22)]">
              <MessageSquare className="ml-2 hidden h-4 w-4 shrink-0 text-stone-500 sm:block" />
              <input
                type="text"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                disabled={isBusy || connectionStatus !== '已连接 AppServer'}
                placeholder={connectionStatus === '已连接 AppServer' ? 'Ask DeepCoder' : 'AppServer disconnected'}
                className="h-10 min-w-0 flex-1 bg-transparent px-2 text-sm text-stone-100 outline-none placeholder:text-stone-600 disabled:cursor-not-allowed"
              />
              {isBusy ? (
                <button
                  type="button"
                  onClick={handleStop}
                  disabled={!activeTurnIdRef.current || isCancelling}
                  title={isCancelling ? '正在停止' : '停止本轮'}
                  className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-red-300/30 bg-red-500/15 text-red-100 transition hover:bg-red-500/25 disabled:cursor-not-allowed disabled:opacity-45"
                >
                  <Square className="h-3.5 w-3.5" />
                </button>
              ) : (
                <button
                  type="submit"
                  disabled={!input.trim() || connectionStatus !== '已连接 AppServer'}
                  className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-lime-300/35 bg-lime-300 text-black transition hover:bg-lime-200 disabled:cursor-not-allowed disabled:border-[#30362d] disabled:bg-[#20251d] disabled:text-stone-600"
                >
                  <Send className="h-4 w-4" />
                </button>
              )}
            </form>
          </footer>
        </main>

        <aside className="hidden w-80 shrink-0 border-l border-[#2a2f28] bg-[#11140f]/95 xl:flex xl:flex-col">
          <div className="flex h-16 items-center justify-between border-b border-[#2a2f28] px-4">
            <div className="flex items-center gap-2 text-sm font-semibold text-stone-100">
              <Gauge className="h-4 w-4 text-lime-200" />
              Details
            </div>
            <span className="rounded-md border border-[#30362d] px-2 py-1 text-[10px] uppercase tracking-wide text-stone-500">
              {isBusy ? 'live' : 'idle'}
            </span>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto no-scrollbar">
            <section className="border-b border-[#2a2f28] p-4">
              <div className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-stone-500">
                <Brain className="h-4 w-4 text-amber-200" />
                Reasoning
              </div>
              <div className="min-h-28 rounded-lg border border-[#30362d] bg-[#151913] p-3 text-sm leading-6 text-stone-300">
                <div className="max-h-72 overflow-y-auto whitespace-pre-wrap break-words no-scrollbar">
                  {reasoningText || <span className="text-stone-600">暂无推理内容</span>}
                </div>
              </div>
            </section>

            <section className="border-b border-[#2a2f28] p-4">
              <div className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-stone-500">
                <Wrench className="h-4 w-4 text-cyan-200" />
                Tools
              </div>
              <div className="space-y-2">
                {toolEvents.length === 0 ? (
                  <div className="rounded-lg border border-dashed border-[#30362d] px-3 py-4 text-sm text-stone-600">
                    暂无工具调用
                  </div>
                ) : toolEvents.map((tool, index) => (
                  <div key={`${tool.id}-${index}`} className={`rounded-lg border px-3 py-2 ${toolTone(tool.tone)}`}>
                    <div className="flex items-center justify-between gap-3">
                      <span className="truncate text-sm font-medium">{tool.label}</span>
                      <span className="shrink-0 text-[10px] uppercase tracking-wide">{tool.detail}</span>
                    </div>
                  </div>
                ))}
              </div>
            </section>

            <section className="p-4">
              <div className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-stone-500">
                <Gauge className="h-4 w-4 text-lime-200" />
                Tokens
              </div>
              <div className="grid grid-cols-3 gap-2">
                {[
                  ['Input', tokenUsage?.input_tokens],
                  ['Output', tokenUsage?.output_tokens],
                  ['Total', tokenUsage?.total_tokens],
                ].map(([label, value]) => (
                  <div key={label} className="rounded-lg border border-[#30362d] bg-[#151913] px-2 py-3 text-center">
                    <p className="text-[10px] uppercase tracking-wide text-stone-600">{label}</p>
                    <p className="mt-1 text-sm font-semibold text-stone-100">{value ?? '-'}</p>
                  </div>
                ))}
              </div>
            </section>
          </div>
        </aside>
      </div>
    </div>
  );
}
