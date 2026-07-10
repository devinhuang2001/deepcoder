//! AppServer transports.

use anyhow::{Result, bail};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{self, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_hdr_async_with_config;
use tokio_tungstenite::tungstenite::{
    Message,
    handshake::server::{ErrorResponse, Request, Response},
    http::{HeaderValue, StatusCode, header},
    protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
};
use url::Url;

use crate::protocol::{JsonRpcRequest, JsonRpcResponse, MessageProcessor, NotificationSender};

pub struct InProcessClient {
    tx: mpsc::Sender<InProcessRequest>,
}

struct InProcessRequest {
    request: String,
    response_tx: oneshot::Sender<String>,
}

struct RunningTurn {
    start_request_id: Option<Value>,
    handle: JoinHandle<()>,
}

const ACCESS_TOKEN_ENV: &str = "DEEPCODER_ACCESS_TOKEN";
const ALLOWED_ORIGINS_ENV: &str = "DEEPCODER_ALLOWED_ORIGINS";
const APPLICATION_ENV: &str = "DEEPCODER_ENV";
const REQUIRE_AUTH_ENV: &str = "DEEPCODER_REQUIRE_AUTH";
const WEBSOCKET_PROTOCOL: &str = "deepcoder-v1";
const TOKEN_PROTOCOL_PREFIX: &str = "deepcoder-token.";
const MIN_ACCESS_TOKEN_LEN: usize = 32;
const MAX_ACCESS_TOKEN_LEN: usize = 256;
const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_WEBSOCKET_FRAME_BYTES: usize = 256 * 1024;
const MAX_WEBSOCKET_CONNECTIONS: usize = 32;
const MAX_MESSAGES_PER_MINUTE: usize = 120;
const MAX_RUNNING_TURNS_PER_CONNECTION: usize = 4;
const OUTGOING_QUEUE_CAPACITY: usize = 64;
const NOTIFICATION_QUEUE_CAPACITY: usize = 128;

#[derive(Clone)]
pub struct WebSocketSecurity {
    access_token: Option<String>,
    allowed_origins: Vec<String>,
}

impl WebSocketSecurity {
    pub fn new(access_token: Option<String>, allowed_origins: Vec<String>) -> Result<Self> {
        Self::new_with_requirement(access_token, allowed_origins, false)
    }

    fn new_with_requirement(
        access_token: Option<String>,
        allowed_origins: Vec<String>,
        require_authentication: bool,
    ) -> Result<Self> {
        let access_token = access_token.map(|token| token.trim().to_string());

        if require_authentication && access_token.is_none() {
            bail!("{ACCESS_TOKEN_ENV} is required in production");
        }

        if let Some(token) = access_token.as_deref()
            && (token.len() < MIN_ACCESS_TOKEN_LEN
                || token.len() > MAX_ACCESS_TOKEN_LEN
                || !token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
        {
            bail!(
                "{ACCESS_TOKEN_ENV} must be {MIN_ACCESS_TOKEN_LEN}-{MAX_ACCESS_TOKEN_LEN} URL-safe characters (A-Z, a-z, 0-9, - or _)"
            );
        }

        let allowed_origins = allowed_origins
            .into_iter()
            .filter(|origin| !origin.trim().is_empty())
            .map(|origin| canonical_origin(&origin))
            .collect::<Result<Vec<_>>>()?;
        if access_token.is_some() && allowed_origins.is_empty() {
            bail!(
                "{ALLOWED_ORIGINS_ENV} must contain at least one exact http(s) origin when authentication is enabled"
            );
        }

        Ok(Self {
            access_token,
            allowed_origins,
        })
    }

    pub fn from_env() -> Result<Self> {
        let access_token = optional_env(ACCESS_TOKEN_ENV)?;
        let allowed_origins = optional_env(ALLOWED_ORIGINS_ENV)?
            .map(|origins| origins.split(',').map(str::to_string).collect())
            .unwrap_or_default();
        let production = optional_env(APPLICATION_ENV)?
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("production"));
        let require_authentication = optional_env(REQUIRE_AUTH_ENV)?.is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        });
        Self::new_with_requirement(
            access_token,
            allowed_origins,
            production || require_authentication,
        )
    }

    pub fn authentication_enabled(&self) -> bool {
        self.access_token.is_some()
    }

    pub fn credentials_are_valid(&self, offered_protocols: Option<&str>) -> bool {
        let protocols = offered_protocols
            .into_iter()
            .flat_map(|protocols| protocols.split(','))
            .map(str::trim)
            .filter(|protocol| !protocol.is_empty())
            .collect::<Vec<_>>();
        if protocols
            .iter()
            .filter(|protocol| **protocol == WEBSOCKET_PROTOCOL)
            .count()
            != 1
        {
            return false;
        }
        let provided_tokens = protocols
            .iter()
            .filter_map(|protocol| protocol.strip_prefix(TOKEN_PROTOCOL_PREFIX))
            .collect::<Vec<_>>();
        let Some(expected_token) = self.access_token.as_deref() else {
            return provided_tokens.is_empty();
        };
        provided_tokens.len() == 1
            && constant_time_eq(provided_tokens[0].as_bytes(), expected_token.as_bytes())
    }

    pub fn origin_is_allowed(&self, _host: Option<&str>, origin: Option<&str>) -> bool {
        let Some(origin) = origin.and_then(|value| canonical_origin(value).ok()) else {
            return !self.authentication_enabled() && origin.is_none();
        };
        if self
            .allowed_origins
            .iter()
            .any(|allowed| allowed == &origin)
        {
            return true;
        }
        !self.authentication_enabled() && is_loopback_origin(&origin)
    }
}

fn optional_env(name: &str) -> Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => bail!("{name} must be valid Unicode"),
    }
}

impl fmt::Debug for WebSocketSecurity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WebSocketSecurity")
            .field("authentication_enabled", &self.authentication_enabled())
            .field("allowed_origins", &self.allowed_origins)
            .finish()
    }
}

fn canonical_origin(origin: &str) -> Result<String> {
    let parsed = Url::parse(origin.trim())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || !matches!(parsed.path(), "" | "/")
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        bail!("invalid WebSocket origin: expected an exact http(s) origin");
    }
    Ok(parsed.origin().ascii_serialization().to_ascii_lowercase())
}

fn is_loopback_origin(origin: &str) -> bool {
    let Ok(parsed) = Url::parse(origin) else {
        return false;
    };
    parsed.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

pub fn validate_websocket_listener(addr: SocketAddr, security: &WebSocketSecurity) -> Result<()> {
    if !addr.ip().is_loopback() && !security.authentication_enabled() {
        bail!(
            "refusing to expose unauthenticated AppServer on {addr}; set {ACCESS_TOKEN_ENV} to a strong URL-safe token"
        );
    }
    Ok(())
}

struct MessageRateLimiter {
    limit: usize,
    window: Duration,
    messages: VecDeque<Instant>,
}

impl MessageRateLimiter {
    fn new(limit: usize, window: Duration) -> Self {
        Self {
            limit,
            window,
            messages: VecDeque::with_capacity(limit),
        }
    }

    fn allow(&mut self, now: Instant) -> bool {
        while self.messages.front().is_some_and(|timestamp| {
            now.checked_duration_since(*timestamp)
                .is_some_and(|elapsed| elapsed >= self.window)
        }) {
            self.messages.pop_front();
        }
        if self.messages.len() >= self.limit {
            return false;
        }
        self.messages.push_back(now);
        true
    }
}

pub fn inprocess_client(config: deepcoder_config::Config) -> InProcessClient {
    let (tx, mut rx) = mpsc::channel::<InProcessRequest>(32);
    tokio::spawn(async move {
        let mut processor = MessageProcessor::new(config);
        while let Some(request) = rx.recv().await {
            let response = processor.process_json(&request.request).await;
            let text = serde_json::to_string(&response).unwrap_or_else(|error| {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32603,
                        "message": format!("Internal error: {error}")
                    }
                })
                .to_string()
            });
            let _ = request.response_tx.send(text);
        }
    });

    InProcessClient { tx }
}

impl InProcessClient {
    pub async fn request_json(&self, request: Value) -> Result<Value> {
        self.request_text(request.to_string()).await
    }

    pub async fn request_text(&self, request: String) -> Result<Value> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(InProcessRequest {
                request,
                response_tx,
            })
            .await?;
        let response = response_rx.await?;
        Ok(serde_json::from_str(&response)?)
    }
}

pub async fn run_stdio(config: deepcoder_config::Config) -> Result<()> {
    process_json_lines(config, io::stdin(), io::stdout()).await
}

pub async fn process_json_lines<R, W>(
    config: deepcoder_config::Config,
    reader: R,
    writer: W,
) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut writer = writer;
    let mut processor = MessageProcessor::new(config);
    while let Some(line) = lines.next_line().await? {
        let response = processor.process_json(&line).await;
        let mut bytes = serde_json::to_vec(&response)?;
        bytes.push(b'\n');
        writer.write_all(&bytes).await?;
        writer.flush().await?;
    }
    Ok(())
}

pub async fn run_ws_server(config: deepcoder_config::Config, addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    run_ws_server_on_listener(config, listener).await
}

pub async fn run_ws_server_on_listener(
    config: deepcoder_config::Config,
    listener: TcpListener,
) -> Result<()> {
    let security = WebSocketSecurity::from_env()?;
    run_ws_server_on_listener_with_security(config, listener, security).await
}

#[allow(
    clippy::result_large_err,
    reason = "tungstenite handshake callbacks require ErrorResponse by value"
)]
async fn run_ws_server_on_listener_with_security(
    config: deepcoder_config::Config,
    listener: TcpListener,
    security: WebSocketSecurity,
) -> Result<()> {
    let addr = listener.local_addr()?;
    validate_websocket_listener(addr, &security)?;
    tracing::info!(
        authentication = security.authentication_enabled(),
        "AppServer WebSocket listening on {addr}"
    );
    let connection_limit = Arc::new(Semaphore::new(MAX_WEBSOCKET_CONNECTIONS));

    loop {
        let (stream, peer) = listener.accept().await?;
        tracing::info!("AppServer WebSocket client connected: {peer}");
        let Ok(connection_permit) = connection_limit.clone().try_acquire_owned() else {
            tracing::warn!("AppServer WebSocket connection limit reached; rejecting {peer}");
            drop(stream);
            continue;
        };
        let config = config.clone();
        let security = security.clone();
        tokio::spawn(async move {
            let handshake_security = security.clone();
            let callback = move |request: &Request, mut response: Response| {
                let headers = request.headers();
                let origin_headers = headers.get_all(header::ORIGIN);
                let mut origins = origin_headers.iter();
                let origin = origins.next().and_then(|value| value.to_str().ok());
                if origins.next().is_some() {
                    return Err(handshake_error(
                        StatusCode::FORBIDDEN,
                        "WebSocket origin is not allowed",
                    ));
                }
                let protocol_headers = headers.get_all(header::SEC_WEBSOCKET_PROTOCOL);
                let mut protocol_values = protocol_headers.iter();
                let protocols = protocol_values.next().and_then(|value| value.to_str().ok());
                if protocol_values.next().is_some() {
                    return Err(handshake_error(
                        StatusCode::UNAUTHORIZED,
                        "WebSocket authentication failed",
                    ));
                }

                if !handshake_security.origin_is_allowed(None, origin) {
                    return Err(handshake_error(
                        StatusCode::FORBIDDEN,
                        "WebSocket origin is not allowed",
                    ));
                }
                if !handshake_security.credentials_are_valid(protocols) {
                    return Err(handshake_error(
                        StatusCode::UNAUTHORIZED,
                        "WebSocket authentication failed",
                    ));
                }
                if protocol_is_offered(protocols, WEBSOCKET_PROTOCOL) {
                    response.headers_mut().insert(
                        header::SEC_WEBSOCKET_PROTOCOL,
                        HeaderValue::from_static(WEBSOCKET_PROTOCOL),
                    );
                }
                Ok(response)
            };
            let websocket_config = WebSocketConfig {
                max_message_size: Some(MAX_WEBSOCKET_MESSAGE_BYTES),
                max_frame_size: Some(MAX_WEBSOCKET_FRAME_BYTES),
                ..WebSocketConfig::default()
            };
            let handshake = accept_hdr_async_with_config(stream, callback, Some(websocket_config));
            let ws_stream = match tokio::time::timeout(Duration::from_secs(5), handshake).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(err)) => {
                    tracing::warn!("WebSocket handshake failed: {err}");
                    return;
                }
                Err(_) => {
                    tracing::warn!("WebSocket handshake timed out for {peer}");
                    return;
                }
            };
            let (mut sink, mut stream) = ws_stream.split();
            let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<Message>(OUTGOING_QUEUE_CAPACITY);
            let writer = tokio::spawn(async move {
                while let Some(message) = outgoing_rx.recv().await {
                    if let Err(err) = sink.send(message).await {
                        tracing::warn!("WebSocket write failed: {err}");
                        break;
                    }
                }
            });
            let (notification_tx, mut notification_rx) =
                mpsc::channel::<Value>(NOTIFICATION_QUEUE_CAPACITY);
            let notification_outgoing_tx = outgoing_tx.clone();
            let notifications = tokio::spawn(async move {
                while let Some(notification) = notification_rx.recv().await {
                    let text = notification.to_string();
                    if notification_outgoing_tx
                        .send(Message::Text(text))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });
            let processor = Arc::new(Mutex::new(MessageProcessor::new(config)));
            let mut running_turns = HashMap::<String, RunningTurn>::new();
            let mut rate_limiter =
                MessageRateLimiter::new(MAX_MESSAGES_PER_MINUTE, Duration::from_secs(60));
            while let Some(message) = stream.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        if !rate_limiter.allow(Instant::now()) {
                            tracing::warn!("WebSocket message rate limit exceeded for {peer}");
                            let _ = outgoing_tx
                                .send(Message::Close(Some(CloseFrame {
                                    code: CloseCode::Policy,
                                    reason: "message rate limit exceeded".into(),
                                })))
                                .await;
                            break;
                        }
                        if !process_ws_text(
                            text,
                            processor.clone(),
                            notification_tx.clone(),
                            outgoing_tx.clone(),
                            &mut running_turns,
                        )
                        .await
                        {
                            break;
                        }
                    }
                    Ok(Message::Binary(_)) => {
                        let _ = outgoing_tx
                            .send(Message::Close(Some(CloseFrame {
                                code: CloseCode::Unsupported,
                                reason: "binary JSON-RPC frames are not supported".into(),
                            })))
                            .await;
                        break;
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Ping(bytes)) => {
                        if outgoing_tx.send(Message::Pong(bytes)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Message::Pong(_)) => {}
                    Err(err) => {
                        tracing::warn!("WebSocket read failed: {err}");
                        break;
                    }
                    _ => {}
                }
            }
            for (_, running) in running_turns {
                running.handle.abort();
            }
            drop(notification_tx);
            notifications.abort();
            let _ = notifications.await;
            drop(outgoing_tx);
            let _ = tokio::time::timeout(Duration::from_secs(1), writer).await;
            drop(connection_permit);
        });
    }
}

fn protocol_is_offered(protocols: Option<&str>, expected: &str) -> bool {
    protocols
        .into_iter()
        .flat_map(|values| values.split(','))
        .map(str::trim)
        .any(|protocol| protocol == expected)
}

fn handshake_error(status: StatusCode, message: &str) -> ErrorResponse {
    Response::builder()
        .status(status)
        .body(Some(message.to_string()))
        .expect("static WebSocket error response must be valid")
}

async fn process_ws_text(
    text: String,
    processor: Arc<Mutex<MessageProcessor>>,
    notifications: NotificationSender,
    outgoing_tx: mpsc::Sender<Message>,
    running_turns: &mut HashMap<String, RunningTurn>,
) -> bool {
    running_turns.retain(|_, running| !running.handle.is_finished());

    let Ok(mut request) = serde_json::from_str::<JsonRpcRequest>(&text) else {
        let response = {
            let mut processor = processor.lock().await;
            processor
                .process_json_with_notifications(&text, Some(notifications))
                .await
        };
        return send_json_response(&outgoing_tx, response).await;
    };

    match request.method.as_str() {
        "turn/start" => {
            let turn_id = ensure_turn_id(&mut request);
            if running_turns.len() >= MAX_RUNNING_TURNS_PER_CONNECTION {
                return send_json_response(
                    &outgoing_tx,
                    JsonRpcResponse::error(
                        request.id.clone(),
                        -32011,
                        "Too many running turns on this connection",
                    ),
                )
                .await;
            }
            if running_turns.contains_key(&turn_id) {
                return send_json_response(
                    &outgoing_tx,
                    JsonRpcResponse::error(
                        request.id.clone(),
                        -32009,
                        format!("Turn already running: {turn_id}"),
                    ),
                )
                .await;
            }

            let start_request_id = request.id.clone();
            let processor_task = processor.clone();
            let notifications_task = notifications.clone();
            let outgoing_task = outgoing_tx.clone();
            let handle = tokio::spawn(async move {
                let response = {
                    let mut processor = processor_task.lock().await;
                    processor
                        .process_with_notifications(request, Some(notifications_task))
                        .await
                };
                let _ = send_json_response(&outgoing_task, response).await;
            });
            running_turns.insert(
                turn_id,
                RunningTurn {
                    start_request_id,
                    handle,
                },
            );
            true
        }
        "turn/cancel" => {
            let Some(turn_id) = turn_id_from_params(request.params.as_ref()).map(str::to_string)
            else {
                let response = {
                    let mut processor = processor.lock().await;
                    processor
                        .process_with_notifications(request, Some(notifications))
                        .await
                };
                return send_json_response(&outgoing_tx, response).await;
            };

            if let Some(running) = running_turns.remove(&turn_id) {
                running.handle.abort();
                let start_cancelled = JsonRpcResponse::error(
                    running.start_request_id,
                    -32010,
                    format!("Turn cancelled: {turn_id}"),
                );
                if !send_json_response(&outgoing_tx, start_cancelled).await {
                    return false;
                }
                send_cancel_notification(&notifications, &turn_id).await;
                return send_json_response(
                    &outgoing_tx,
                    JsonRpcResponse::success(
                        request.id.clone(),
                        serde_json::json!({
                            "turn_id": turn_id,
                            "cancelled": true,
                            "running": true
                        }),
                    ),
                )
                .await;
            }

            let response = {
                let mut processor = processor.lock().await;
                processor
                    .process_with_notifications(request, Some(notifications))
                    .await
            };
            send_json_response(&outgoing_tx, response).await
        }
        _ => {
            let response = {
                let mut processor = processor.lock().await;
                processor
                    .process_with_notifications(request, Some(notifications))
                    .await
            };
            send_json_response(&outgoing_tx, response).await
        }
    }
}

async fn send_json_response(
    outgoing_tx: &mpsc::Sender<Message>,
    response: JsonRpcResponse,
) -> bool {
    match serde_json::to_string(&response) {
        Ok(text) => outgoing_tx.send(Message::Text(text)).await.is_ok(),
        Err(err) => {
            tracing::warn!("WebSocket response serialization failed: {err}");
            false
        }
    }
}

async fn send_cancel_notification(notifications: &NotificationSender, turn_id: &str) {
    let _ = notifications
        .send(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "turn/event",
            "params": {
                "type": "turn_cancelled",
                "turn_id": turn_id
            }
        }))
        .await;
}

fn ensure_turn_id(request: &mut JsonRpcRequest) -> String {
    if let Some(turn_id) = turn_id_from_params(request.params.as_ref()) {
        return turn_id.to_string();
    }

    let turn_id = uuid::Uuid::new_v4().to_string();
    match request.params.as_mut() {
        Some(Value::Object(params)) => {
            params.insert("turn_id".into(), Value::String(turn_id.clone()));
        }
        _ => {
            request.params = Some(serde_json::json!({
                "turn_id": turn_id
            }));
        }
    }
    turn_id
}

fn turn_id_from_params(params: Option<&Value>) -> Option<&str> {
    let params = params?;
    params
        .get("turn_id")
        .or_else(|| params.get("turnId"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::time::{Duration, timeout};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{Message, client::IntoClientRequest},
    };

    fn temp_config(name: &str) -> deepcoder_config::Config {
        let path = std::env::temp_dir().join(format!(
            "deepcoder_app_transport_{name}_{}",
            std::process::id()
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        std::fs::create_dir_all(&path).unwrap();
        let mut config = deepcoder_config::Config::load_default().unwrap();
        config.system.data_dir = path;
        config
    }

    const TEST_ACCESS_TOKEN: &str = "test_access_token_0123456789abcdef";
    const TEST_ORIGIN: &str = "https://deepcoder.example.com";

    fn secured_websocket_security() -> WebSocketSecurity {
        WebSocketSecurity::new(Some(TEST_ACCESS_TOKEN.into()), vec![TEST_ORIGIN.into()]).unwrap()
    }

    #[test]
    fn websocket_security_requires_a_strong_url_safe_token() {
        assert!(WebSocketSecurity::new(Some("short".into()), Vec::new()).is_err());
        assert!(
            WebSocketSecurity::new(
                Some("this token contains spaces and is not safe".into()),
                vec![TEST_ORIGIN.into()],
            )
            .is_err()
        );
        assert!(WebSocketSecurity::new(Some("   ".into()), vec![TEST_ORIGIN.into()]).is_err());
        assert!(WebSocketSecurity::new(Some(TEST_ACCESS_TOKEN.into()), Vec::new()).is_err());
        assert!(secured_websocket_security().authentication_enabled());
        assert!(WebSocketSecurity::new_with_requirement(None, Vec::new(), true).is_err());
        assert!(
            WebSocketSecurity::new(
                Some(TEST_ACCESS_TOKEN.into()),
                vec!["https://user@example.com/path?query=1".into()],
            )
            .is_err()
        );
    }

    #[test]
    fn websocket_security_rejects_missing_or_wrong_credentials() {
        let security = secured_websocket_security();

        assert!(!security.credentials_are_valid(None));
        assert!(!security.credentials_are_valid(Some("deepcoder-v1")));
        assert!(!security.credentials_are_valid(Some(
            "deepcoder-v1, deepcoder-token.wrong_token_0123456789abcdef"
        )));
        assert!(security.credentials_are_valid(Some(&format!(
            "deepcoder-v1, deepcoder-token.{TEST_ACCESS_TOKEN}"
        ))));
        assert!(!security.credentials_are_valid(Some(&format!(
            "deepcoder-v1, deepcoder-token.{TEST_ACCESS_TOKEN}, deepcoder-token.{TEST_ACCESS_TOKEN}"
        ))));

        let local_security = WebSocketSecurity::new(None, Vec::new()).unwrap();
        assert!(!local_security.credentials_are_valid(None));
        assert!(local_security.credentials_are_valid(Some("deepcoder-v1")));
    }

    #[test]
    fn websocket_security_requires_exact_allowed_origin() {
        let security = secured_websocket_security();

        assert!(security.origin_is_allowed(Some("deepcoder.example.com"), Some(TEST_ORIGIN)));
        assert!(!security.origin_is_allowed(
            Some("deepcoder.example.com"),
            Some("https://attacker.example")
        ));
        assert!(!security.origin_is_allowed(Some("deepcoder.example.com"), None));
    }

    #[test]
    fn websocket_security_honors_explicit_origin_allowlist() {
        let security = WebSocketSecurity::new(
            Some(TEST_ACCESS_TOKEN.into()),
            vec!["https://console.example.com".into()],
        )
        .unwrap();

        assert!(
            security
                .origin_is_allowed(Some("api.example.com"), Some("https://console.example.com"))
        );
        assert!(!security.origin_is_allowed(
            Some("api.example.com"),
            Some("https://console.example.com.evil.test")
        ));
    }

    #[test]
    fn websocket_local_mode_rejects_cross_site_browser_origins() {
        let security = WebSocketSecurity::new(None, Vec::new()).unwrap();

        assert!(security.origin_is_allowed(None, None));
        assert!(security.origin_is_allowed(Some("127.0.0.1:8080"), Some("http://127.0.0.1:5173")));
        assert!(security.origin_is_allowed(Some("localhost:8080"), Some("http://localhost:5173")));
        assert!(
            !security.origin_is_allowed(Some("127.0.0.1:8080"), Some("https://attacker.example"))
        );
    }

    #[test]
    fn websocket_security_debug_output_redacts_the_access_token() {
        let security = secured_websocket_security();
        let debug = format!("{security:?}");

        assert!(!debug.contains(TEST_ACCESS_TOKEN));
        assert!(debug.contains("authentication_enabled"));
    }

    #[test]
    fn websocket_public_listener_fails_closed_without_authentication() {
        let public_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080);
        let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let unsecured = WebSocketSecurity::new(None, Vec::new()).unwrap();
        let secured = secured_websocket_security();

        assert!(validate_websocket_listener(public_addr, &unsecured).is_err());
        assert!(validate_websocket_listener(public_addr, &secured).is_ok());
        assert!(validate_websocket_listener(local_addr, &unsecured).is_ok());
    }

    #[test]
    fn websocket_rate_limiter_rejects_messages_over_the_window_limit() {
        let start = std::time::Instant::now();
        let mut limiter = MessageRateLimiter::new(2, Duration::from_secs(60));

        assert!(limiter.allow(start));
        assert!(limiter.allow(start + Duration::from_secs(1)));
        assert!(!limiter.allow(start + Duration::from_secs(2)));
        assert!(limiter.allow(start + Duration::from_secs(61)));
    }

    #[tokio::test]
    async fn websocket_handshake_enforces_origin_and_token() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(run_ws_server_on_listener_with_security(
            temp_config("websocket_auth"),
            listener,
            secured_websocket_security(),
        ));

        let mut missing_token = format!("ws://{addr}/ws").into_client_request().unwrap();
        missing_token
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static(TEST_ORIGIN));
        missing_token.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(WEBSOCKET_PROTOCOL),
        );
        let error = connect_async(missing_token).await.unwrap_err();
        assert!(error.to_string().contains("401"));

        let mut authorized = format!("ws://{addr}/ws").into_client_request().unwrap();
        authorized
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static(TEST_ORIGIN));
        authorized.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_str(&format!(
                "{WEBSOCKET_PROTOCOL}, {TOKEN_PROTOCOL_PREFIX}{TEST_ACCESS_TOKEN}"
            ))
            .unwrap(),
        );
        let (mut ws, response) = connect_async(authorized).await.unwrap();
        assert_eq!(
            response
                .headers()
                .get(header::SEC_WEBSOCKET_PROTOCOL)
                .and_then(|value| value.to_str().ok()),
            Some(WEBSOCKET_PROTOCOL)
        );
        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize"
            })
            .to_string(),
        ))
        .await
        .unwrap();
        let response = ws.next().await.unwrap().unwrap();
        let response: Value = serde_json::from_str(response.to_text().unwrap()).unwrap();
        assert_eq!(response["result"]["server"], "deepcoder-app-server");

        handle.abort();
    }

    #[tokio::test]
    async fn websocket_rejects_binary_json_rpc_frames_with_close_1003() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(run_ws_server_on_listener_with_security(
            temp_config("websocket_binary"),
            listener,
            WebSocketSecurity::new(None, Vec::new()).unwrap(),
        ));
        let mut request = format!("ws://{addr}").into_client_request().unwrap();
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(WEBSOCKET_PROTOCOL),
        );
        let (mut ws, _) = connect_async(request).await.unwrap();

        ws.send(Message::Binary(vec![0xff, 0x00])).await.unwrap();
        let close = timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Close(Some(frame)) = close else {
            panic!("expected an unsupported-data close frame, got {close:?}");
        };
        assert_eq!(frame.code, CloseCode::Unsupported);

        handle.abort();
    }

    #[tokio::test]
    async fn websocket_rate_limit_closes_with_policy_violation() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(run_ws_server_on_listener_with_security(
            temp_config("websocket_rate_limit"),
            listener,
            WebSocketSecurity::new(None, Vec::new()).unwrap(),
        ));
        let mut request = format!("ws://{addr}").into_client_request().unwrap();
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(WEBSOCKET_PROTOCOL),
        );
        let (mut ws, _) = connect_async(request).await.unwrap();

        for id in 0..=MAX_MESSAGES_PER_MINUTE {
            ws.send(Message::Text(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "initialize"
                })
                .to_string(),
            ))
            .await
            .unwrap();
        }

        let close_code = timeout(Duration::from_secs(5), async {
            loop {
                match ws.next().await {
                    Some(Ok(Message::Close(Some(frame)))) => break frame.code,
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => panic!("unexpected WebSocket error: {error}"),
                    None => panic!("connection ended without a close frame"),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(close_code, CloseCode::Policy);

        handle.abort();
    }

    async fn spawn_hanging_http_server()
    -> (String, oneshot::Receiver<()>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = oneshot::channel();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let _ = accepted_tx.send(());
            let mut buffer = [0_u8; 1024];
            let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buffer).await;
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        (format!("http://{addr}"), accepted_rx, handle)
    }

    #[tokio::test]
    async fn inprocess_transport_round_trip() {
        let client = inprocess_client(temp_config("round_trip"));
        let response = client
            .request_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize"
            }))
            .await
            .unwrap();
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["server"], "deepcoder-app-server");
    }

    #[tokio::test]
    async fn inprocess_transport_returns_parse_error() {
        let client = inprocess_client(temp_config("parse_error"));
        let response = client.request_text("{not-json}".into()).await.unwrap();
        assert_eq!(response["error"]["code"], -32700);
    }

    #[tokio::test]
    async fn stdio_transport_round_trip() {
        let input = br#"{"jsonrpc":"2.0","id":1,"method":"initialize"}
"#;
        let mut output = Vec::new();
        process_json_lines(temp_config("stdio"), &input[..], &mut output)
            .await
            .unwrap();
        let text = String::from_utf8(output).unwrap();
        let response: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["server"], "deepcoder-app-server");
    }

    #[tokio::test]
    async fn websocket_transport_round_trip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(run_ws_server_on_listener(
            temp_config("websocket"),
            listener,
        ));
        let mut request = format!("ws://{addr}").into_client_request().unwrap();
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(WEBSOCKET_PROTOCOL),
        );
        let (mut ws, _) = connect_async(request).await.unwrap();
        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize"
            })
            .to_string(),
        ))
        .await
        .unwrap();
        let message = ws.next().await.unwrap().unwrap();
        let response: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["server"], "deepcoder-app-server");
        handle.abort();
    }

    #[tokio::test]
    async fn websocket_turn_cancel_aborts_running_turn() {
        let (base_url, accepted_rx, http_handle) = spawn_hanging_http_server().await;
        let mut config = temp_config("websocket_cancel");
        config.api_key = Some("test-key".into());
        config.provider.base_url = base_url;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_handle = tokio::spawn(run_ws_server_on_listener(config, listener));
        let mut request = format!("ws://{addr}").into_client_request().unwrap();
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(WEBSOCKET_PROTOCOL),
        );
        let (mut ws, _) = connect_async(request).await.unwrap();

        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "thread/create"
            })
            .to_string(),
        ))
        .await
        .unwrap();
        let message = ws.next().await.unwrap().unwrap();
        let created: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        let thread_id = created["result"]["thread"]["id"].as_str().unwrap();

        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "turn/start",
                "params": {
                    "thread_id": thread_id,
                    "turn_id": "client-turn-cancel",
                    "input": "hold this turn open"
                }
            })
            .to_string(),
        ))
        .await
        .unwrap();
        timeout(Duration::from_secs(5), accepted_rx)
            .await
            .unwrap()
            .unwrap();

        ws.send(Message::Text(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "turn/cancel",
                "params": {
                    "turn_id": "client-turn-cancel"
                }
            })
            .to_string(),
        ))
        .await
        .unwrap();

        let mut saw_start_error = false;
        let mut saw_cancel_response = false;
        let mut saw_cancel_notification = false;
        for _ in 0..4 {
            let message = timeout(Duration::from_secs(5), ws.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let response: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
            if response["id"] == 2 {
                saw_start_error = response["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("cancelled");
            } else if response["id"] == 3 {
                saw_cancel_response = response["result"]["cancelled"] == true
                    && response["result"]["running"] == true;
            } else if response["method"] == "turn/event" {
                saw_cancel_notification = response["params"]["type"] == "turn_cancelled"
                    && response["params"]["turn_id"] == "client-turn-cancel";
            }

            if saw_start_error && saw_cancel_response && saw_cancel_notification {
                break;
            }
        }

        assert!(
            saw_start_error,
            "turn/start caller should receive a cancellation error"
        );
        assert!(
            saw_cancel_response,
            "turn/cancel should acknowledge the running turn"
        );
        assert!(
            saw_cancel_notification,
            "clients should receive a cancellation event"
        );

        server_handle.abort();
        http_handle.abort();
    }
}
