//! Stdio transport integration test (task 9.4).
//!
//! Launches the real `agentmem` binary as a child process speaking MCP over
//! stdio, performs `initialize` + `tools/list` + a `write`→`read` round-trip, and
//! asserts a clean shutdown. A successful round-trip also proves stdout carries
//! only well-formed JSON-RPC frames: any stray log byte on stdout would corrupt
//! the stream and break frame parsing (task 9.2).

use rmcp::model::CallToolRequestParams;
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

    // tools/list returns the ten core tools plus recall_memory_notes, which the
    // default `simple` recall backend enables.
    let tools = service.list_tools(Default::default()).await.unwrap();
    assert_eq!(tools.tools.len(), 11);
    assert!(tools.tools.iter().any(|t| t.name == "recall_memory_notes"));

    // write then read a memory note round-trip.
    service
        .call_tool(
            CallToolRequestParams::new("write_memory_note").with_arguments(
                json!({
                    "agent": "jarvis", "user": "tony",
                    "path": "Agents/topics/note.md", "content": "hello stdio"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();

    let read = service
        .call_tool(
            CallToolRequestParams::new("read_memory_note").with_arguments(
                json!({
                    "agent": "jarvis", "user": "tony",
                    "path": "Agents/topics/note.md"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();

    let structured = read.structured_content.expect("structured content");
    assert_eq!(structured["content"], "hello stdio");

    // Clean shutdown.
    service.cancel().await.unwrap();
}
