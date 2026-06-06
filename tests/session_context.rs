//! Integration coverage for the session-context resource and prompt surfaces
//! (change `configurable-session-context`, tasks 7.3).
//!
//! Drives the real `agentmem` binary over stdio and exercises
//! `resources/templates/list` + `resources/read` and `prompts/list` +
//! `prompts/get`, including the empty-vault case and a VFS-scheme variation.

use rmcp::model::{
    CallToolRequestParams, GetPromptRequestParams, PromptMessageContent, ReadResourceRequestParams,
    ResourceContents,
};
use rmcp::service::ServiceExt;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::json;
use tokio::process::Command;

/// Launch the server over stdio with the given VFS scheme.
async fn serve(
    tmp: &assert_fs::TempDir,
    scheme: &str,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let bin = env!("CARGO_BIN_EXE_agentmem");
    ().serve(
        TokioChildProcess::new(Command::new(bin).configure(|cmd| {
            cmd.env("AGENTMEM_ROOT_DIR", tmp.path());
            cmd.env("AGENTMEM_TRANSPORT", "stdio");
            cmd.env("AGENTMEM_VFS_SCHEME", scheme);
        }))
        .unwrap(),
    )
    .await
    .expect("server should initialize")
}

fn resource_text(result: &rmcp::model::ReadResourceResult) -> String {
    match &result.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => text.clone(),
        _ => panic!("expected text resource contents"),
    }
}

fn prompt_text(result: &rmcp::model::GetPromptResult) -> String {
    match &result.messages[0].content {
        PromptMessageContent::Text { text } => text.clone(),
        _ => panic!("expected text prompt content"),
    }
}

#[tokio::test]
async fn resource_template_and_read_render_context() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let service = serve(&tmp, "<agent>.<user>").await;

    // Seed a foundational file for coder/alice.
    service
        .call_tool(
            CallToolRequestParams::new("evolve_core_persona").with_arguments(
                json!({"agent":"coder","user":"alice","which":"persona","content":"PERSONA-BODY"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();

    // resources/templates/list → URI params follow the scheme.
    let templates = service
        .list_resource_templates(Default::default())
        .await
        .unwrap();
    assert_eq!(templates.resource_templates.len(), 1);
    assert_eq!(
        templates.resource_templates[0].uri_template,
        "agentmem://session-context/{agent}/{user}"
    );

    // resources/read for a populated scope renders the persona body.
    let read = service
        .read_resource(ReadResourceRequestParams::new(
            "agentmem://session-context/coder/alice",
        ))
        .await
        .unwrap();
    let text = resource_text(&read);
    assert!(text.contains("PERSONA-BODY"));
    assert!(text.contains("# Session Context"));

    // Empty-vault scope still succeeds, with the missing sentinel.
    let read_empty = service
        .read_resource(ReadResourceRequestParams::new(
            "agentmem://session-context/coder/bob",
        ))
        .await
        .unwrap();
    assert!(resource_text(&read_empty).contains("(not yet recorded"));

    service.cancel().await.unwrap();
}

#[tokio::test]
async fn prompt_lists_args_and_renders() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let service = serve(&tmp, "<agent>.<user>").await;

    service
        .call_tool(
            CallToolRequestParams::new("evolve_core_persona").with_arguments(
                json!({"agent":"coder","user":"alice","which":"persona","content":"PROMPT-PERSONA"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();

    // prompts/list → required args follow the scheme.
    let prompts = service.list_prompts(Default::default()).await.unwrap();
    assert_eq!(prompts.prompts.len(), 1);
    let p = &prompts.prompts[0];
    assert_eq!(p.name, "session-context");
    let arg_names: Vec<&str> = p
        .arguments
        .as_ref()
        .unwrap()
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert_eq!(arg_names, vec!["agent", "user"]);

    // prompts/get renders the context for the supplied scope.
    let got = service
        .get_prompt(
            GetPromptRequestParams::new("session-context").with_arguments(
                json!({"agent":"coder","user":"alice"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert!(prompt_text(&got).contains("PROMPT-PERSONA"));

    // Missing required argument is rejected.
    let err = service
        .get_prompt(
            GetPromptRequestParams::new("session-context")
                .with_arguments(json!({"agent":"coder"}).as_object().unwrap().clone()),
        )
        .await;
    assert!(err.is_err(), "missing scope arg should error");

    service.cancel().await.unwrap();
}

#[tokio::test]
async fn surfaces_follow_a_custom_scheme() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let service = serve(&tmp, "<agent>").await;

    let templates = service
        .list_resource_templates(Default::default())
        .await
        .unwrap();
    assert_eq!(
        templates.resource_templates[0].uri_template,
        "agentmem://session-context/{agent}"
    );

    let prompts = service.list_prompts(Default::default()).await.unwrap();
    let arg_names: Vec<&str> = prompts.prompts[0]
        .arguments
        .as_ref()
        .unwrap()
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert_eq!(arg_names, vec!["agent"]);

    // A single-segment read resolves under the one-key scheme.
    let read = service
        .read_resource(ReadResourceRequestParams::new(
            "agentmem://session-context/coder",
        ))
        .await
        .unwrap();
    assert!(resource_text(&read).contains("# Session Context"));

    service.cancel().await.unwrap();
}
