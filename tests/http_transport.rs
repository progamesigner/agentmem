//! HTTP transport integration tests (tasks 10.5–10.7).
//!
//! Each test spawns the real `agentmem` binary as a child process bound to a
//! loopback (or wildcard) address and drives it over HTTP.

use std::process::Stdio;
use std::time::Duration;

use rmcp::service::ServiceExt;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// Spawn the binary with the given bind and optional bearer. `bind` of `None`
/// exercises the default `127.0.0.1:8000`. Stderr is piped so warnings can be
/// inspected.
fn spawn(root: &std::path::Path, bind: Option<&str>, bearer: Option<&str>) -> Child {
    let bin = env!("CARGO_BIN_EXE_agentmem");
    let mut cmd = Command::new(bin);
    cmd.env("AGENTMEM_ROOT_DIR", root)
        .env("AGENTMEM_TRANSPORT", "http")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(b) = bind {
        cmd.env("AGENTMEM_HTTP_BIND", b);
    }
    if let Some(token) = bearer {
        cmd.env("AGENTMEM_HTTP_BEARER", token);
    }
    cmd.spawn().expect("spawn agentmem")
}

/// Poll `GET /health` until it succeeds or the timeout elapses.
async fn wait_health(base: &str) {
    let client = reqwest::Client::new();
    for _ in 0..100 {
        if let Ok(resp) = client.get(format!("{base}/health")).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("server did not become healthy at {base}");
}

#[tokio::test]
async fn http_default_bind_health_and_mcp_roundtrip() {
    let tmp = assert_fs::TempDir::new().unwrap();
    // AGENTMEM_ROOT_DIR is the only override → default bind 127.0.0.1:8000.
    let mut child = spawn(tmp.path(), None, None);
    let base = "http://127.0.0.1:8000";

    wait_health(base).await;

    // MCP initialize + tools/list over Streamable HTTP.
    let transport = StreamableHttpClientTransport::with_client(
        reqwest::Client::default(),
        StreamableHttpClientTransportConfig::with_uri(format!("{base}/mcp")),
    );
    let service = ().serve(transport).await.expect("mcp initialize");
    let tools = service.list_tools(Default::default()).await.unwrap();
    assert_eq!(tools.tools.len(), 9);
    service.cancel().await.unwrap();

    child.kill().await.unwrap();
}

#[tokio::test]
async fn http_unauthenticated_request_is_rejected_when_bearer_set() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18651";
    let mut child = spawn(tmp.path(), Some(bind), Some("s3cret"));
    let base = format!("http://{bind}");

    // /health is unauthenticated, so it is the readiness probe.
    wait_health(&base).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/mcp"))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .body(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);

    child.kill().await.unwrap();
}

#[tokio::test]
async fn http_non_loopback_without_bearer_warns_on_startup() {
    let tmp = assert_fs::TempDir::new().unwrap();
    // Wildcard bind, ephemeral port, no bearer → startup WARN expected.
    let mut child = spawn(tmp.path(), Some("0.0.0.0:0"), None);

    let stderr = child.stderr.take().unwrap();
    let mut lines = BufReader::new(stderr).lines();

    let found = tokio::time::timeout(Duration::from_secs(5), async {
        while let Ok(Some(line)) = lines.next_line().await {
            if line.contains("WARN") && line.contains("AGENTMEM_HTTP_BEARER") {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    child.kill().await.unwrap();
    assert!(found, "expected a startup WARN naming AGENTMEM_HTTP_BEARER");
}
