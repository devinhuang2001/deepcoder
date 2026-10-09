use std::sync::Arc;

use async_trait::async_trait;
use deepcoder_tools::traits::{ToolApprovalRequest, ToolApprover};
use deepcoder_types::event::EngineEvent;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::state::DesktopApprovalPrompt;

pub enum DesktopRuntimeEvent {
    Engine(EngineEvent),
    Approval(DesktopApprovalPrompt),
    TurnFinished {
        session: Box<deepcoder_engine::Session>,
        result: Result<(), String>,
    },
}

#[derive(Clone)]
pub struct DesktopApprover {
    tx: mpsc::UnboundedSender<DesktopRuntimeEvent>,
}

impl DesktopApprover {
    pub fn new(tx: mpsc::UnboundedSender<DesktopRuntimeEvent>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl ToolApprover for DesktopApprover {
    async fn approve(&self, request: ToolApprovalRequest) -> bool {
        let (respond_to, response) = oneshot::channel();
        let prompt = DesktopApprovalPrompt::new(request, respond_to);
        if self.tx.send(DesktopRuntimeEvent::Approval(prompt)).is_err() {
            return false;
        }
        response.await.unwrap_or(false)
    }
}

pub fn new_session(
    config: deepcoder_config::Config,
    tx: mpsc::UnboundedSender<DesktopRuntimeEvent>,
) -> deepcoder_engine::Session {
    let router = Arc::new(deepcoder_tools::ToolRouter::with_builtins());
    let mut session = deepcoder_engine::Session::new(config, router);
    session.set_tool_approver(Some(Arc::new(DesktopApprover::new(tx))));
    session
}

pub fn resume_session(
    config: deepcoder_config::Config,
    persistence: &deepcoder_persistence::Persistence,
    session_id: uuid::Uuid,
    fork: bool,
    tx: mpsc::UnboundedSender<DesktopRuntimeEvent>,
) -> deepcoder_error::DeepCoderResult<Option<deepcoder_engine::Session>> {
    let router = Arc::new(deepcoder_tools::ToolRouter::with_builtins());
    let mut session = if fork {
        deepcoder_engine::Session::fork_from(config, router, persistence, session_id)?
    } else {
        deepcoder_engine::Session::resume(config, router, persistence, session_id)?
    };
    if let Some(session) = session.as_mut() {
        session.set_tool_approver(Some(Arc::new(DesktopApprover::new(tx))));
    }
    Ok(session)
}

pub fn spawn_turn(
    runtime: &tokio::runtime::Runtime,
    mut session: deepcoder_engine::Session,
    input: String,
    tx: mpsc::UnboundedSender<DesktopRuntimeEvent>,
) -> JoinHandle<()> {
    runtime.spawn(async move {
        let (event_tx, mut event_rx) = broadcast::channel(1024);
        let event_forward_tx = tx.clone();
        let forward_handle = tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let _ = event_forward_tx.send(DesktopRuntimeEvent::Engine(event));
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });

        let result = deepcoder_engine::turn::run_turn(&mut session, &input, event_tx)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        let _ = forward_handle.await;
        let _ = tx.send(DesktopRuntimeEvent::TurnFinished {
            session: Box::new(session),
            result,
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn desktop_approver_round_trips_decision() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let approver = DesktopApprover::new(tx);
        let handle = tokio::spawn(async move {
            approver
                .approve(ToolApprovalRequest {
                    tool_name: "bash".into(),
                    message: "approve".into(),
                    arguments: serde_json::json!({"command": "Get-Date"}),
                })
                .await
        });

        let DesktopRuntimeEvent::Approval(mut prompt) = rx.recv().await.unwrap() else {
            panic!("expected approval prompt");
        };
        assert_eq!(prompt.tool_name, "bash");
        prompt.resolve(true);
        assert!(handle.await.unwrap());
    }

    #[test]
    fn spawn_turn_streams_mock_provider_events() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let body = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"hello desktop\"}}]}\n\ndata: [DONE]\n\n";
            let (base_url, server_handle) =
                spawn_http_server(http_response("200 OK", "text/event-stream", body)).await;
            let mut config = deepcoder_config::Config::load_default().unwrap();
            config.api_key = Some("test-key".into());
            config.provider.base_url = base_url;
            config.system.data_dir = std::env::temp_dir().join(format!(
                "deepcoder_desktop_runner_{}",
                uuid::Uuid::new_v4()
            ));
            let (tx, mut rx) = mpsc::unbounded_channel();
            let session = new_session(config, tx.clone());
            let handle = spawn_turn(&runtime, session, "hello".into(), tx);

            let mut saw_text = false;
            let mut saw_reasoning = false;
            let mut saw_finished = false;
            while let Some(event) = rx.recv().await {
                match event {
                    DesktopRuntimeEvent::Engine(EngineEvent::TextDelta { content, .. }) => {
                        saw_text |= content == "hello desktop";
                    }
                    DesktopRuntimeEvent::Engine(EngineEvent::ReasoningDelta { content, .. }) => {
                        saw_reasoning |= content == "think";
                    }
                    DesktopRuntimeEvent::TurnFinished { result, .. } => {
                        assert!(result.is_ok());
                        saw_finished = true;
                        break;
                    }
                    _ => {}
                }
            }

            handle.await.unwrap();
            let requests = server_handle.await.unwrap();
            assert!(requests[0].starts_with("POST /chat/completions"));
            assert!(saw_text);
            assert!(saw_reasoning);
            assert!(saw_finished);
        });
    }

    fn http_response(status: &str, content_type: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    async fn spawn_http_server(response: String) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 8192];
            let size = socket.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..size]).to_string();
            socket.write_all(response.as_bytes()).await.unwrap();
            vec![request]
        });

        (format!("http://{addr}"), handle)
    }
}
