//! HTTP transport integration tests (tasks 10.5–10.7).
//!
//! Each test spawns the real `agentmem` binary as a child process bound to a
//! loopback (or wildcard) address and drives it over HTTP.

use std::process::Stdio;
use std::time::Duration;

use rmcp::model::CallToolRequestParams;
use rmcp::service::ServiceExt;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// Spawn the binary with the given bind and optional bearer. `bind` of `None`
/// exercises the default `127.0.0.1:8000`. Stderr is piped so warnings can be
/// inspected.
fn spawn(root: &std::path::Path, bind: Option<&str>, bearer: Option<&str>) -> Child {
    spawn_full(root, bind, bearer, None)
}

/// Like [`spawn`], with an optional `AGENTMEM_HTTP_ALLOWED_HOSTS` value.
fn spawn_full(
    root: &std::path::Path,
    bind: Option<&str>,
    bearer: Option<&str>,
    allowed_hosts: Option<&str>,
) -> Child {
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
    if let Some(hosts) = allowed_hosts {
        cmd.env("AGENTMEM_HTTP_ALLOWED_HOSTS", hosts);
    }
    cmd.spawn().expect("spawn agentmem")
}

/// `POST /mcp` carrying an explicit `Host` header, returning the HTTP status.
/// rmcp's DNS-rebinding gate answers `403` before any MCP processing when the
/// `Host` is not allowed; an allowed `Host` falls through to MCP handling
/// (which, for this minimal body, is any non-`403` status).
async fn mcp_post_status(base: &str, host: &str) -> u16 {
    reqwest::Client::new()
        .post(format!("{base}/mcp"))
        .header("Host", host)
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .body(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
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

// --- Host validation (AGENTMEM_HTTP_ALLOWED_HOSTS) --------------------------

#[tokio::test]
async fn http_rejects_non_loopback_host_by_default() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18661";
    let mut child = spawn(tmp.path(), Some(bind), None);
    let base = format!("http://{bind}");
    wait_health(&base).await; // /health is not Host-gated.

    // Default allow-list is loopback only, so a cluster DNS Host is rejected.
    let status = mcp_post_status(&base, "agentmem.svc.cluster.local").await;
    assert_eq!(
        status, 403,
        "non-loopback Host should be forbidden by default"
    );

    child.kill().await.unwrap();
}

#[tokio::test]
async fn http_accepts_allowlisted_host() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18662";
    let mut child = spawn_full(
        tmp.path(),
        Some(bind),
        None,
        Some("agentmem.svc.cluster.local"),
    );
    let base = format!("http://{bind}");
    wait_health(&base).await;

    // The allow-listed Host clears the DNS-rebinding gate (non-403).
    let status = mcp_post_status(&base, "agentmem.svc.cluster.local").await;
    assert_ne!(status, 403, "allow-listed Host should pass validation");

    // A Host outside the list is still rejected.
    let status = mcp_post_status(&base, "evil.example.net").await;
    assert_eq!(status, 403, "non-listed Host should remain forbidden");

    child.kill().await.unwrap();
}

#[tokio::test]
async fn http_wildcard_accepts_any_host() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18663";
    let mut child = spawn_full(tmp.path(), Some(bind), None, Some("*"));
    let base = format!("http://{bind}");
    wait_health(&base).await;

    // With validation disabled, an arbitrary Host clears the gate.
    let status = mcp_post_status(&base, "anything.example.org").await;
    assert_ne!(status, 403, "wildcard should accept any Host");

    child.kill().await.unwrap();
}

#[tokio::test]
async fn http_loopback_host_accepted_when_unset() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18664";
    let mut child = spawn(tmp.path(), Some(bind), None);
    let base = format!("http://{bind}");
    wait_health(&base).await;

    // The unchanged default still accepts loopback.
    let status = mcp_post_status(&base, "127.0.0.1:18664").await;
    assert_ne!(status, 403, "loopback Host must remain accepted by default");

    child.kill().await.unwrap();
}

// --- GET /v1/context -------------------------------------------------------

#[tokio::test]
async fn context_endpoint_renders_markdown_bootstrap() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18652";
    let mut child = spawn(tmp.path(), Some(bind), None);
    let base = format!("http://{bind}");
    wait_health(&base).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/v1/context?agent=default&user=alice"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    assert!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/markdown"),
        "expected text/markdown content type"
    );
    let body = resp.text().await.unwrap();
    // A fresh vault renders the compiled-in default template.
    assert!(body.contains("# Session Context"));
    assert!(body.contains("<AGENTMEM:TOOLS>"));

    child.kill().await.unwrap();
}

#[tokio::test]
async fn context_endpoint_json_negotiation_reports_missing() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18653";
    let mut child = spawn(tmp.path(), Some(bind), None);
    let base = format!("http://{bind}");
    wait_health(&base).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/v1/context?agent=default&user=alice"))
        .header("Accept", "application/json")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    assert!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["rendered"]
            .as_str()
            .unwrap()
            .contains("# Session Context")
    );
    // A fresh vault has all five foundational files absent.
    let missing = body["missing"].as_array().unwrap();
    assert_eq!(missing.len(), 5);
    assert!(missing.iter().any(|v| v == "PERSONA.md"));
    assert!(missing.iter().any(|v| v == "MEMORY.md"));

    child.kill().await.unwrap();
}

#[tokio::test]
async fn context_endpoint_rejects_invalid_scope() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18654";
    let mut child = spawn(tmp.path(), Some(bind), None);
    let base = format!("http://{bind}");
    wait_health(&base).await;

    let client = reqwest::Client::new();

    // Missing placeholder `user`.
    let resp = client
        .get(format!("{base}/v1/context?agent=default"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("user"));

    // Empty value for `user`.
    let resp = client
        .get(format!("{base}/v1/context?agent=default&user="))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("user"));

    // Unexpected parameter `role`.
    let resp = client
        .get(format!(
            "{base}/v1/context?agent=default&user=alice&role=admin"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("role"));

    child.kill().await.unwrap();
}

#[tokio::test]
async fn context_endpoint_honours_bearer() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18655";
    let mut child = spawn(tmp.path(), Some(bind), Some("s3cret"));
    let base = format!("http://{bind}");
    wait_health(&base).await; // /health stays reachable without auth.

    let client = reqwest::Client::new();
    let url = format!("{base}/v1/context?agent=default&user=alice");

    // No bearer → 401.
    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 401);

    // Matching bearer → 200.
    let resp = client
        .get(&url)
        .header("Authorization", "Bearer s3cret")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    assert!(resp.text().await.unwrap().contains("# Session Context"));

    child.kill().await.unwrap();
}

#[tokio::test]
async fn context_endpoint_matches_load_session_context_tool() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bind = "127.0.0.1:18656";
    let mut child = spawn(tmp.path(), Some(bind), None);
    let base = format!("http://{bind}");
    wait_health(&base).await;

    // Rendered markdown from the plain HTTP endpoint.
    let client = reqwest::Client::new();
    let http_body = client
        .get(format!("{base}/v1/context?agent=default&user=alice"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    // Rendered text from the `load_session_context` MCP tool for the same scope.
    let transport = StreamableHttpClientTransport::with_client(
        reqwest::Client::default(),
        StreamableHttpClientTransportConfig::with_uri(format!("{base}/mcp")),
    );
    let service = ().serve(transport).await.expect("mcp initialize");
    let mut arguments = serde_json::Map::new();
    arguments.insert("agent".into(), serde_json::json!("default"));
    arguments.insert("user".into(), serde_json::json!("alice"));
    let result = service
        .call_tool(CallToolRequestParams::new("load_session_context").with_arguments(arguments))
        .await
        .unwrap();
    let tool_rendered = result.structured_content.as_ref().unwrap()["rendered"]
        .as_str()
        .unwrap()
        .to_string();
    service.cancel().await.unwrap();

    assert_eq!(
        http_body, tool_rendered,
        "the /v1/context body must be byte-identical to the tool result"
    );

    child.kill().await.unwrap();
}
