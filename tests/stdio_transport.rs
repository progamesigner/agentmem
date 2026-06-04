//! Stdio transport integration test (task 9.4).
//!
//! Launches the real `agentmem` binary as a child process speaking MCP over
//! stdio, performs `initialize` + `tools/list` + a `write`→`read` round-trip, and
//! asserts a clean shutdown. A successful round-trip also proves stdout carries
//! only well-formed JSON-RPC frames: any stray log byte on stdout would corrupt
//! the stream and break frame parsing (task 9.2).

use rmcp::model::CallToolRequestParam;
use rmcp::service::ServiceExt;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::json;
use tokio::process::Command;

#[tokio::test]
async fn stdio_initialize_list_and_roundtrip() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let bin = env!("CARGO_BIN_EXE_agentmem");

    let service = ()
        .serve(
            TokioChildProcess::new(Command::new(bin).configure(|cmd| {
                cmd.env("AGENTMEM_ROOT_DIR", tmp.path());
                cmd.env("AGENTMEM_TRANSPORT", "stdio");
            }))
            .unwrap(),
        )
        .await
        .expect("server should initialize");

    // initialize handshake populated peer info.
    assert!(service.peer_info().is_some());

    // tools/list returns the full set of nine tools.
    let tools = service.list_tools(Default::default()).await.unwrap();
    assert_eq!(tools.tools.len(), 9);

    // write then read a memory note round-trip.
    service
        .call_tool(CallToolRequestParam {
            name: "write_memory_note".into(),
            arguments: json!({
                "agent": "coder", "user": "alice",
                "path": "Agents/PERSONA.md", "content": "hello stdio"
            })
            .as_object()
            .cloned(),
        })
        .await
        .unwrap();

    let read = service
        .call_tool(CallToolRequestParam {
            name: "read_memory_note".into(),
            arguments: json!({
                "agent": "coder", "user": "alice",
                "path": "Agents/PERSONA.md"
            })
            .as_object()
            .cloned(),
        })
        .await
        .unwrap();

    let structured = read.structured_content.expect("structured content");
    assert_eq!(structured["content"], "hello stdio");

    // Clean shutdown.
    service.cancel().await.unwrap();
}
