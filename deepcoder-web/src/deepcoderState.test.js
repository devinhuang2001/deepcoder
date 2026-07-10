import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildThreadTranscript,
  calculateReconnectDelay,
  createWebSocketProtocols,
  createClientTurnId,
  extractDiagnostics,
  messageToBubble,
  resolveWebSocketUrl,
} from './deepcoderState.js';

test('message_to_bubble_renders_text_only', () => {
  const bubble = messageToBubble({
    id: 'message-1',
    role: 'Assistant',
    contents: [
      { Reasoning: { content: 'private chain' } },
      { Text: 'visible answer' },
      { ToolCall: { id: 'call-1', name: 'read_file', arguments: { path: 'README.md' } } },
      { ToolResult: { id: 'call-1', content: 'ok', is_error: false } },
    ],
  });

  assert.deepEqual(bubble, {
    id: 'message-1',
    role: 'assistant',
    content: 'visible answer',
  });
});

test('message_to_bubble_omits_non_text_only_messages', () => {
  const bubble = messageToBubble({
    id: 'message-2',
    role: 'Tool',
    contents: [
      { ToolResult: { id: 'call-1', content: 'ok', is_error: false } },
    ],
  });

  assert.equal(bubble, null);
});

test('extract_diagnostics_from_messages', () => {
  const diagnostics = extractDiagnostics([
    {
      role: 'Assistant',
      contents: [
        { Reasoning: { content: 'think one' } },
        { ToolCall: { id: 'call-1', name: 'read_file', arguments: { path: 'README.md' } } },
      ],
    },
    {
      role: 'Tool',
      contents: [
        { ToolResult: { id: 'call-1', content: 'ok', is_error: false } },
        { ToolResult: { id: 'call-2', content: 'bad', is_error: true } },
      ],
    },
    {
      role: 'Assistant',
      contents: [
        { Reasoning: { content: 'think two' } },
      ],
    },
  ]);

  assert.equal(diagnostics.reasoningText, 'think one\n\nthink two');
  assert.deepEqual(diagnostics.toolEvents, [
    { id: 'call-1', label: 'read_file', detail: '请求执行', tone: 'blue' },
    { id: 'call-1', label: 'call-1', detail: '完成', tone: 'emerald' },
    { id: 'call-2', label: 'call-2', detail: '失败', tone: 'rose' },
  ]);
});

test('build_thread_transcript_restores_visible_messages', () => {
  const transcript = buildThreadTranscript(
    { id: '12345678-aaaa-bbbb-cccc-123456789abc' },
    [
      { id: 'u1', role: 'User', contents: [{ Text: 'hello' }] },
      { id: 'r1', role: 'Assistant', contents: [{ Reasoning: { content: 'hidden' } }] },
      { id: 'a1', role: 'Assistant', contents: [{ Text: 'hi' }] },
    ],
    1700000000000,
  );

  assert.deepEqual(transcript, [
    {
      id: 'system-thread-12345678-aaaa-bbbb-cccc-123456789abc-1700000000000',
      role: 'system',
      content: '已载入会话 12345678',
    },
    { id: 'u1', role: 'user', content: 'hello' },
    { id: 'a1', role: 'assistant', content: 'hi' },
  ]);
});

test('build_thread_transcript_returns_empty_session_hint', () => {
  const transcript = buildThreadTranscript(
    { id: 'empty-thread' },
    [],
    1700000000000,
  );

  assert.deepEqual(transcript, [
    {
      id: 'system-thread-empty-thread-1700000000000',
      role: 'system',
      content: '已载入会话 empty-th',
    },
    { id: 'assistant-empty-empty-thread', role: 'assistant', content: '这个会话还没有消息。' },
  ]);
});

test('create_client_turn_id_uses_crypto_when_available', () => {
  const turnId = createClientTurnId({
    crypto: {
      randomUUID: () => 'turn-from-crypto',
    },
  });

  assert.equal(turnId, 'turn-from-crypto');
});

test('resolve_websocket_url_uses_secure_same_origin_endpoint_in_production', () => {
  assert.equal(
    resolveWebSocketUrl(undefined, {
      protocol: 'https:',
      host: 'deepcoder.example.com',
    }),
    'wss://deepcoder.example.com/ws',
  );
});

test('resolve_websocket_url_honors_explicit_build_configuration', () => {
  assert.equal(
    resolveWebSocketUrl('wss://api.example.com/socket', {
      protocol: 'https:',
      host: 'deepcoder.example.com',
    }),
    'wss://api.example.com/socket',
  );
});

test('create_websocket_protocols_keeps_token_in_memory_only', () => {
  assert.deepEqual(createWebSocketProtocols(), ['deepcoder-v1']);
  assert.deepEqual(
    createWebSocketProtocols('test_access_token_0123456789abcdef'),
    ['deepcoder-v1', 'deepcoder-token.test_access_token_0123456789abcdef'],
  );
  assert.throws(() => createWebSocketProtocols('too short'), /32/);
});

test('calculate_reconnect_delay_uses_capped_exponential_backoff', () => {
  assert.equal(calculateReconnectDelay(0, () => 0), 1000);
  assert.equal(calculateReconnectDelay(3, () => 0), 8000);
  assert.equal(calculateReconnectDelay(20, () => 0), 30000);
  assert.equal(calculateReconnectDelay(0, () => 1), 1250);
});
