use async_trait::async_trait;
use deepcoder_error::DeepCoderError;
use deepcoder_tools::{ToolContext, ToolRouter};
use deepcoder_types::tool::ToolCall;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

fn test_config() -> deepcoder_config::Config {
    deepcoder_config::Config::load_default().expect("default config loads")
}

fn temp_dir(name: &str) -> PathBuf {
    let unique = format!("deepcoder_tools_test_{}_{}", name, std::process::id());
    let path = std::env::temp_dir().join(unique);
    if path.exists() {
        std::fs::remove_dir_all(&path).ok();
    }
    std::fs::create_dir_all(&path).expect("create temp dir");
    path
}

async fn execute(
    router: &Arc<ToolRouter>,
    tool_name: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let ctx = ToolContext {
        config: test_config(),
        workspace_root: Some(std::env::temp_dir()),
        tool_router: Some(router.clone()),
        tool_approver: None,
    };
    router
        .execute(
            &ToolCall {
                call_id: format!("call_{tool_name}"),
                tool_name: tool_name.to_string(),
                arguments,
            },
            &ctx,
        )
        .await
        .expect("tool succeeds")
        .data
}

async fn execute_err(
    router: &Arc<ToolRouter>,
    tool_name: &str,
    arguments: serde_json::Value,
) -> DeepCoderError {
    let ctx = ToolContext {
        config: test_config(),
        workspace_root: Some(std::env::temp_dir()),
        tool_router: Some(router.clone()),
        tool_approver: None,
    };
    router
        .execute(
            &ToolCall {
                call_id: format!("call_{tool_name}"),
                tool_name: tool_name.to_string(),
                arguments,
            },
            &ctx,
        )
        .await
        .expect_err("tool fails")
}

struct StaticApprover(bool);

#[async_trait]
impl deepcoder_tools::traits::ToolApprover for StaticApprover {
    async fn approve(&self, _request: deepcoder_tools::traits::ToolApprovalRequest) -> bool {
        self.0
    }
}

#[tokio::test]
async fn builtin_direct_specs_include_core_tools() {
    let router = ToolRouter::with_builtins();
    let mut names: Vec<_> = router
        .direct_specs()
        .await
        .into_iter()
        .map(|spec| spec.name)
        .collect();
    names.sort();

    for expected in [
        "agent",
        "bash",
        "edit_file",
        "glob",
        "grep",
        "read_file",
        "task_create",
        "task_delete",
        "task_list",
        "task_update",
        "web_fetch",
        "web_search",
        "write_file",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "missing {expected}"
        );
    }
}

#[tokio::test]
async fn task_tools_create_list_update_and_delete() {
    let router = Arc::new(ToolRouter::with_builtins());
    let created = execute(
        &router,
        "task_create",
        serde_json::json!({"title": "write specs", "notes": "keep it testable"}),
    )
    .await;
    let task_id = created["task"]["id"].as_str().unwrap().to_string();
    assert_eq!(created["task"]["status"], "todo");

    let listed = execute(&router, "task_list", serde_json::json!({})).await;
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);

    let updated = execute(
        &router,
        "task_update",
        serde_json::json!({"id": task_id, "status": "done"}),
    )
    .await;
    let task_id = updated["task"]["id"].as_str().unwrap().to_string();
    assert_eq!(updated["task"]["status"], "done");

    let filtered = execute(&router, "task_list", serde_json::json!({"status": "done"})).await;
    assert_eq!(filtered["tasks"].as_array().unwrap().len(), 1);

    let deleted = execute(&router, "task_delete", serde_json::json!({"id": task_id})).await;
    assert_eq!(deleted["deleted"], true);
    let listed = execute(&router, "task_list", serde_json::json!({})).await;
    assert!(listed["tasks"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn agent_tool_spawns_isolated_task_contract() {
    let router = Arc::new(ToolRouter::with_builtins());
    let output = execute(
        &router,
        "agent",
        serde_json::json!({"task": "review module", "context": "core-engine", "max_turns": 2}),
    )
    .await;
    assert_eq!(output["isolated"], true);
    assert_eq!(output["status"], "queued");
    assert!(output["agent_session_id"].as_str().unwrap().len() > 20);

    let listed = execute(
        &router,
        "task_list",
        serde_json::json!({"status": "queued"}),
    )
    .await;
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn file_tools_write_read_and_edit() {
    let router = Arc::new(ToolRouter::with_builtins());
    let dir = temp_dir("file");
    let file = dir.join("note.txt");

    execute(
        &router,
        "write_file",
        serde_json::json!({"path": file.display().to_string(), "content": "alpha\nbeta\n"}),
    )
    .await;
    let read = execute(
        &router,
        "read_file",
        serde_json::json!({"path": file.display().to_string(), "max_bytes": 1024}),
    )
    .await;
    assert_eq!(read["content"], "alpha\nbeta\n");
    assert_eq!(read["lines"][1]["number"], 2);

    let edit = execute(
        &router,
        "edit_file",
        serde_json::json!({"path": file.display().to_string(), "old_string": "beta", "new_string": "gamma"}),
    )
    .await;
    assert_eq!(edit["replacements"], 1);
    assert!(
        std::fs::read_to_string(dir.join("note.txt"))
            .expect("read edited")
            .contains("gamma")
    );
}

#[tokio::test]
async fn file_tools_restrict_paths_to_workspace_root() {
    let router = Arc::new(ToolRouter::with_builtins());
    let workspace = temp_dir("file_workspace");
    let ctx = ToolContext {
        config: test_config(),
        workspace_root: Some(workspace.clone()),
        tool_router: Some(router.clone()),
        tool_approver: None,
    };

    let output = router
        .execute(
            &ToolCall {
                call_id: "call_write_relative".into(),
                tool_name: "write_file".into(),
                arguments: serde_json::json!({
                    "path": "nested/note.txt",
                    "content": "inside workspace"
                }),
            },
            &ctx,
        )
        .await
        .expect("relative write succeeds")
        .data;
    assert!(Path::new(output["path"].as_str().unwrap()).ends_with("nested/note.txt"));
    assert_eq!(
        std::fs::read_to_string(workspace.join("nested").join("note.txt")).unwrap(),
        "inside workspace"
    );

    let outside = workspace
        .parent()
        .unwrap()
        .join(format!("deepcoder_tools_outside_{}", std::process::id()));
    let denied = router
        .execute(
            &ToolCall {
                call_id: "call_write_outside".into(),
                tool_name: "write_file".into(),
                arguments: serde_json::json!({
                    "path": outside.display().to_string(),
                    "content": "outside"
                }),
            },
            &ctx,
        )
        .await
        .expect_err("absolute outside path is denied");
    assert!(matches!(denied, DeepCoderError::ToolDenied { .. }));

    let denied = router
        .execute(
            &ToolCall {
                call_id: "call_read_escape".into(),
                tool_name: "read_file".into(),
                arguments: serde_json::json!({"path": "../escape.txt"}),
            },
            &ctx,
        )
        .await
        .expect_err("parent directory escape is denied");
    assert!(matches!(denied, DeepCoderError::ToolDenied { .. }));
}

#[tokio::test]
async fn file_tools_report_missing_limit_and_ambiguous_edits() {
    let router = Arc::new(ToolRouter::with_builtins());
    let dir = temp_dir("file_errors");
    let file = dir.join("note.txt");
    std::fs::write(&file, "alpha\nbeta\nbeta\n").expect("write file");

    let missing = execute_err(
        &router,
        "read_file",
        serde_json::json!({"path": dir.join("missing.txt").display().to_string()}),
    )
    .await;
    assert!(matches!(missing, DeepCoderError::Io(_)));

    let too_large = execute_err(
        &router,
        "read_file",
        serde_json::json!({"path": file.display().to_string(), "max_bytes": 4}),
    )
    .await;
    assert!(too_large.to_string().contains("file too large"));

    let no_match = execute_err(
        &router,
        "edit_file",
        serde_json::json!({
            "path": file.display().to_string(),
            "old_string": "missing",
            "new_string": "new"
        }),
    )
    .await;
    assert!(no_match.to_string().contains("old_string not found"));

    let multiple = execute_err(
        &router,
        "edit_file",
        serde_json::json!({
            "path": file.display().to_string(),
            "old_string": "beta",
            "new_string": "gamma"
        }),
    )
    .await;
    assert!(multiple.to_string().contains("matched 2 times"));
}

#[tokio::test]
async fn glob_and_grep_return_matches() {
    let router = Arc::new(ToolRouter::with_builtins());
    let dir = temp_dir("search");
    let file = dir.join("src").join("main.txt");
    std::fs::create_dir_all(file.parent().unwrap()).expect("create nested dir");
    std::fs::write(&file, "hello\nneedle here\n").expect("write file");

    let glob_root = dir.display().to_string().replace('\\', "/");
    let pattern = format!("{glob_root}/**/*.txt");
    let glob = execute(
        &router,
        "glob",
        serde_json::json!({"pattern": pattern, "max_results": 10}),
    )
    .await;
    assert!(glob["matches"].as_array().unwrap().iter().any(|value| {
        value
            .as_str()
            .is_some_and(|path| Path::new(path).ends_with("main.txt"))
    }));

    let grep = execute(
        &router,
        "grep",
        serde_json::json!({"path": dir.display().to_string(), "pattern": "needle", "max_results": 10}),
    )
    .await;
    assert_eq!(grep["matches"][0]["line_number"], 2);
    assert_eq!(grep["matches"][0]["line"], "needle here");
}

#[tokio::test]
async fn search_tools_restrict_paths_to_workspace_root() {
    let router = Arc::new(ToolRouter::with_builtins());
    let workspace = temp_dir("search_workspace");
    std::fs::write(workspace.join("inside.txt"), "needle\n").unwrap();
    let outside = workspace
        .parent()
        .unwrap()
        .join(format!("deepcoder_search_outside_{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("outside.txt"), "needle\n").unwrap();
    let ctx = ToolContext {
        config: test_config(),
        workspace_root: Some(workspace.clone()),
        tool_router: Some(router.clone()),
        tool_approver: None,
    };

    let inside = router
        .execute(
            &ToolCall {
                call_id: "call_grep_inside".into(),
                tool_name: "grep".into(),
                arguments: serde_json::json!({"path": ".", "pattern": "needle"}),
            },
            &ctx,
        )
        .await
        .expect("grep inside workspace succeeds")
        .data;
    assert_eq!(inside["matches"].as_array().unwrap().len(), 1);

    let denied = router
        .execute(
            &ToolCall {
                call_id: "call_grep_outside".into(),
                tool_name: "grep".into(),
                arguments: serde_json::json!({
                    "path": outside.display().to_string(),
                    "pattern": "needle"
                }),
            },
            &ctx,
        )
        .await
        .expect_err("grep outside workspace is denied");
    assert!(matches!(denied, DeepCoderError::ToolDenied { .. }));

    let denied = router
        .execute(
            &ToolCall {
                call_id: "call_glob_outside".into(),
                tool_name: "glob".into(),
                arguments: serde_json::json!({
                    "pattern": format!("{}/*.txt", outside.display()),
                }),
            },
            &ctx,
        )
        .await
        .expect_err("glob outside workspace is denied");
    assert!(matches!(denied, DeepCoderError::ToolDenied { .. }));
}

#[tokio::test]
async fn grep_reports_invalid_regex() {
    let router = Arc::new(ToolRouter::with_builtins());
    let error = execute_err(&router, "grep", serde_json::json!({"pattern": "["})).await;
    assert!(error.to_string().contains("invalid regex"));
}

#[tokio::test]
async fn bash_runs_allowed_command() {
    let router = Arc::new(ToolRouter::with_builtins());
    let output = execute(
        &router,
        "bash",
        serde_json::json!({"command": "echo hello", "timeout_ms": 30000}),
    )
    .await;
    assert_eq!(output["success"], true);
    assert!(output["stdout"].as_str().unwrap().contains("hello"));
}

#[tokio::test]
async fn bash_reports_timeout_and_denies_dangerous_command() {
    let router = Arc::new(ToolRouter::with_builtins());
    let timeout_command = if cfg!(target_os = "windows") {
        "Start-Sleep -Milliseconds 200"
    } else {
        "sleep 1"
    };
    let timeout_ctx = ToolContext {
        config: test_config(),
        workspace_root: Some(std::env::temp_dir()),
        tool_router: Some(router.clone()),
        tool_approver: Some(Arc::new(StaticApprover(true))),
    };
    let timeout = router
        .execute(
            &ToolCall {
                call_id: "call_timeout".into(),
                tool_name: "bash".into(),
                arguments: serde_json::json!({"command": timeout_command, "timeout_ms": 1}),
            },
            &timeout_ctx,
        )
        .await
        .expect_err("timeout command fails");
    assert!(timeout.to_string().contains("timed out"));

    let denied = execute_err(&router, "bash", serde_json::json!({"command": "rm -rf /"})).await;
    assert!(matches!(denied, DeepCoderError::ToolDenied { .. }));
}

#[tokio::test]
async fn bash_prompt_requires_explicit_approval() {
    let router = Arc::new(ToolRouter::with_builtins());
    let prompt_command = if cfg!(target_os = "windows") {
        "Get-Date"
    } else {
        "date"
    };

    let denied = execute_err(
        &router,
        "bash",
        serde_json::json!({"command": prompt_command, "timeout_ms": 30000}),
    )
    .await;
    assert!(matches!(denied, DeepCoderError::ToolDenied { .. }));

    let ctx = ToolContext {
        config: test_config(),
        workspace_root: Some(std::env::temp_dir()),
        tool_router: Some(router.clone()),
        tool_approver: Some(Arc::new(StaticApprover(true))),
    };
    let output = router
        .execute(
            &ToolCall {
                call_id: "call_prompt".into(),
                tool_name: "bash".into(),
                arguments: serde_json::json!({"command": prompt_command, "timeout_ms": 30000}),
            },
            &ctx,
        )
        .await
        .expect("approved command runs")
        .data;
    assert_eq!(output["success"], true);
}

#[tokio::test]
async fn bash_uses_configured_sandbox_mode() {
    let router = Arc::new(ToolRouter::with_builtins());
    let mut enforce_config = test_config();
    enforce_config.sandbox.mode = "enforce".into();
    let enforce_ctx = ToolContext {
        config: enforce_config,
        workspace_root: Some(std::env::temp_dir()),
        tool_router: Some(router.clone()),
        tool_approver: None,
    };
    let denied = router
        .execute(
            &ToolCall {
                call_id: "call_enforce".into(),
                tool_name: "bash".into(),
                arguments: serde_json::json!({"command": "echo rm -rf /", "timeout_ms": 30000}),
            },
            &enforce_ctx,
        )
        .await
        .expect_err("sandbox denies dangerous pattern");
    assert!(matches!(denied, DeepCoderError::ToolDenied { .. }));

    let mut off_config = test_config();
    off_config.sandbox.mode = "off".into();
    let off_ctx = ToolContext {
        config: off_config,
        workspace_root: Some(std::env::temp_dir()),
        tool_router: Some(router.clone()),
        tool_approver: None,
    };
    let output = router
        .execute(
            &ToolCall {
                call_id: "call_off".into(),
                tool_name: "bash".into(),
                arguments: serde_json::json!({"command": "echo rm -rf /", "timeout_ms": 30000}),
            },
            &off_ctx,
        )
        .await
        .expect("sandbox off allows command")
        .data;
    assert_eq!(output["success"], true);
}

#[tokio::test]
async fn web_fetch_handles_mock_http_status_truncation_and_timeout() {
    let router = Arc::new(ToolRouter::with_builtins());
    let (base_url, handle) = spawn_http_server(vec![http_response(
        "404 Not Found",
        "text/plain",
        "not found",
    )])
    .await;

    let output = execute(
        &router,
        "web_fetch",
        serde_json::json!({"url": format!("{base_url}/fetch"), "max_bytes": 3}),
    )
    .await;
    assert_eq!(output["status"], 404);
    assert_eq!(output["content"], "not");
    let requests = handle.await.unwrap();
    assert!(requests[0].starts_with("GET /fetch"));

    let (slow_url, slow_handle) = spawn_slow_http_server(Duration::from_millis(200)).await;
    let error = execute_err(
        &router,
        "web_fetch",
        serde_json::json!({"url": format!("{slow_url}/slow"), "timeout_ms": 1}),
    )
    .await;
    assert!(matches!(error, DeepCoderError::Api(_)));
    slow_handle.await.unwrap();
}

#[tokio::test]
async fn web_search_uses_mock_duckduckgo_results() {
    let router = Arc::new(ToolRouter::with_builtins());
    let html = r#"
        <html>
          <a class="result__a" href="https://example.com?a=1&amp;b=2">Title <b>One</b></a>
          <a class="result__a" href="https://example.org">Title Two</a>
        </html>
    "#;
    let (base_url, handle) =
        spawn_http_server(vec![http_response("200 OK", "text/html", html)]).await;

    let output = execute(
        &router,
        "web_search",
        serde_json::json!({
            "query": "deepcoder rust",
            "base_url": format!("{base_url}/search"),
            "max_results": 1
        }),
    )
    .await;
    assert_eq!(output["results"][0]["title"], "Title One");
    assert_eq!(output["results"][0]["url"], "https://example.com?a=1&b=2");

    let requests = handle.await.unwrap();
    assert!(requests[0].starts_with("GET /search?"));
    assert!(requests[0].contains("q=deepcoder+rust"));
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

async fn spawn_http_server(responses: Vec<String>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 8192];
            let size = socket.read(&mut buffer).await.unwrap();
            requests.push(String::from_utf8_lossy(&buffer[..size]).to_string());
            socket.write_all(response.as_bytes()).await.unwrap();
        }
        requests
    });

    (format!("http://{addr}"), handle)
}

async fn spawn_slow_http_server(delay: Duration) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0u8; 8192];
        let _ = socket.read(&mut buffer).await;
        tokio::time::sleep(delay).await;
        let response = http_response("200 OK", "text/plain", "slow");
        let _ = socket.write_all(response.as_bytes()).await;
    });

    (format!("http://{addr}"), handle)
}
