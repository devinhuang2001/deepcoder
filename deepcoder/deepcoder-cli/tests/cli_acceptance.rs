use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "deepcoder_cli_acceptance_{name}_{}_{}",
        std::process::id(),
        uuid_like_suffix()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path).ok();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn uuid_like_suffix() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string()
}

#[test]
fn exec_success_with_mock_provider() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 8192];
        let size = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..size]).to_string();
        let body =
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello cli\"}}]}\n\ndata: [DONE]\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        request
    });

    let dir = temp_dir("exec_success");
    let config_path = dir.join("config.toml");
    let data_dir = dir.join("data").display().to_string().replace('\\', "/");
    std::fs::write(
        &config_path,
        format!(
            r#"
api_key = "test-key"

[provider]
base_url = "http://{addr}"
model = "mock-model"

[system]
data_dir = "{data_dir}"
"#
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_deepcoder"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "exec",
            "say hello",
        ])
        .env_remove("DEEPSEEK_API_KEY")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("hello cli"));
    let request = handle.join().unwrap();
    assert!(request.starts_with("POST /chat/completions"));
}

#[test]
fn missing_api_key_error_is_actionable_from_binary() {
    let dir = temp_dir("missing_key");
    let config_path = dir.join("config.toml");
    let data_dir = dir.join("data").display().to_string().replace('\\', "/");
    std::fs::write(
        &config_path,
        format!(
            r#"
[provider]
base_url = "http://127.0.0.1:9"
model = "mock-model"

[system]
data_dir = "{data_dir}"
"#
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_deepcoder"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "exec",
            "say hello",
        ])
        .env_remove("DEEPSEEK_API_KEY")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("DEEPSEEK_API_KEY"));
}
