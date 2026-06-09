//! In-process tool-handler tests covering every scenario in
//! `specs/memory-tools/spec.md` (task 8.12).
//!
//! The tests drive [`Toolbox`] directly — the same code path the MCP `call_tool`
//! handler uses — so each scenario exercises scope extraction, path resolution,
//! policy gating, visibility filtering, and storage end to end.

use std::sync::Arc;
use std::time::Duration;

use agentmem::AgentmemError;
use agentmem::config::{RecallBackendKind, RecallConfig};
use agentmem::path::PathResolver;
use agentmem::policy::Policy;
use agentmem::recall::RecallEngine;
use agentmem::scheme::Scheme;
use agentmem::storage::Storage;
use agentmem::tools::Toolbox;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use chrono_tz::Tz;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

/// A toolbox with the `simple` recall backend enabled over the same vault.
fn recall_toolbox(tmp: &TempDir) -> Toolbox {
    let mk = || {
        PathResolver::new(
            tmp.path().canonicalize().unwrap(),
            Utf8PathBuf::from("Agents"),
            Scheme::parse("<agent>.<user>").unwrap(),
        )
    };
    let storage = Storage::new(mk(), true, false, &[]);
    let config = RecallConfig {
        backend: RecallBackendKind::Simple,
        watch_debounce: Duration::ZERO,
        regex_scan_byte_cap: usize::MAX,
        max_resident_scopes: 256,
        freshness: Duration::ZERO,
    };
    let recall =
        RecallEngine::new(Arc::new(Storage::new(mk(), true, false, &[])), config).map(Arc::new);
    Toolbox::new(
        storage,
        Policy::Namespaced,
        Tz::UTC,
        tmp.path().join("AGENT_SESSION_CONTEXT.md"),
        recall,
    )
}

fn toolbox(tmp: &TempDir, agents: &str, scheme: &str, policy: Policy) -> Toolbox {
    let resolver = PathResolver::new(
        tmp.path().canonicalize().unwrap(),
        Utf8PathBuf::from(agents),
        Scheme::parse(scheme).unwrap(),
    );
    let storage = Storage::new(resolver, true, false, &[]);
    Toolbox::new(
        storage,
        policy,
        Tz::UTC,
        tmp.path().join("AGENT_SESSION_CONTEXT.md"),
        None,
    )
}

/// Default toolbox: `Agents` folder, `<agent>.<user>` scheme, namespaced.
fn default_tb(tmp: &TempDir) -> Toolbox {
    toolbox(tmp, "Agents", "<agent>.<user>", Policy::Namespaced)
}

fn call(tb: &Toolbox, name: &str, args: Value) -> Result<CallToolResult, AgentmemError> {
    let obj = args.as_object().expect("args must be an object").clone();
    tb.call(name, &obj).expect("tool name must be known")
}

/// Assert the call failed with the given error code.
fn assert_code(res: Result<CallToolResult, AgentmemError>, code: &str) {
    match res {
        Err(e) => assert_eq!(e.code().as_str(), code, "unexpected error variant: {e}"),
        Ok(r) => panic!(
            "expected error {code}, got success: {:?}",
            r.structured_content
        ),
    }
}

fn structured(res: Result<CallToolResult, AgentmemError>) -> Value {
    res.expect("expected success")
        .structured_content
        .expect("structured content")
}

fn write_outside(tmp: &TempDir, rel: &str, content: &str) {
    let path = tmp.path().join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

// --- list_memory_notes ---

#[test]
fn list_includes_own_scope_and_outside_under_namespaced() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/notes.md","content":"x"}),
    )
    .unwrap();
    write_outside(&tmp, "Actions/release.md", "shared");

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(items.contains(&"Agents/topics/notes.md"));
    assert!(items.contains(&"Actions/release.md"));
}

#[test]
fn list_path_prefix_filters_inside_agents_folder() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"x"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/other/go.md","content":"x"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","path_prefix":"topics"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items, vec!["Agents/topics/rust.md"]);
}

#[test]
fn list_hides_other_scopes() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/notes.md","content":"a"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"sam","path":"Agents/topics/notes.md","content":"b"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items, vec!["Agents/topics/notes.md"]);
}

#[test]
fn list_scoped_policy_hides_outside() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Scoped);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/notes.md","content":"x"}),
    )
    .unwrap();
    write_outside(&tmp, "Actions/release.md", "shared");

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items, vec!["Agents/topics/notes.md"]);
}

#[test]
fn list_paginates_with_limit_and_cursor() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for i in 0..5 {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":format!("Agents/topics/n{i}.md"),"content":"x"}),
        )
        .unwrap();
    }
    let page1 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","limit":2}),
    ));
    assert_eq!(page1["items"].as_array().unwrap().len(), 2);
    let cursor = page1["next_cursor"]
        .as_str()
        .expect("cursor on first page")
        .to_string();

    let page2 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","limit":2,"cursor":cursor}),
    ));
    assert_eq!(page2["items"].as_array().unwrap().len(), 2);

    let cursor2 = page2["next_cursor"].as_str().unwrap().to_string();
    let page3 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","limit":2,"cursor":cursor2}),
    ));
    assert_eq!(page3["items"].as_array().unwrap().len(), 1);
    assert!(page3["next_cursor"].is_null());
}

#[test]
fn list_limit_over_max_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "list_memory_notes",
            json!({"agent":"jarvis","user":"tony","limit":1001}),
        ),
        "invalid_argument",
    );
}

#[test]
fn list_ordering_is_stable() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for name in ["c", "a", "b"] {
        call(&tb, "write_memory_note", json!({"agent":"jarvis","user":"tony","path":format!("Agents/topics/{name}.md"),"content":"x"})).unwrap();
    }
    let a = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony"}),
    ));
    let b = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony"}),
    ));
    assert_eq!(a["items"], b["items"]);
}

// --- read_memory_note ---

#[test]
fn read_own_scope_inside() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/note.md","content":"hi"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/note.md"}),
    ));
    assert_eq!(body["content"], "hi");
}

#[test]
fn read_outside_under_namespaced() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "notes");
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Actions/release.md"}),
    ));
    assert_eq!(body["content"], "notes");
}

#[test]
fn read_outside_under_scoped_is_denied() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Scoped);
    write_outside(&tmp, "Actions/release.md", "notes");
    assert_code(
        call(
            &tb,
            "read_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Actions/release.md"}),
        ),
        "path_not_permitted",
    );
}

#[test]
fn read_missing_is_not_found() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "read_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/nope.md"}),
        ),
        "not_found",
    );
}

#[test]
fn read_hidden_is_path_not_permitted() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let res = call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/.secret.md"}),
    );
    assert_code(res, "path_not_permitted");
}

// --- write_memory_note ---

#[test]
fn write_inside_succeeds_with_byte_count() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let body = structured(call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","content":"hello"}),
    ));
    assert_eq!(body["bytes_written"], 5);
}

#[test]
fn write_outside_under_namespaced_is_denied_and_unchanged() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "original");
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Actions/release.md","content":"new"}),
        ),
        "write_denied",
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("Actions/release.md")).unwrap(),
        "original"
    );
}

#[test]
fn write_outside_under_readwrite_succeeds_without_suffix() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Readwrite);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Scratch/team-notes.md","content":"shared"}),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("Scratch/team-notes.md")).unwrap(),
        "shared"
    );
}

#[test]
fn write_under_readonly_is_denied() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Readonly);
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","content":"x"}),
        ),
        "write_denied",
    );
}

#[test]
fn write_hidden_target_is_path_not_permitted_and_creates_nothing() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/.hidden.md","content":"x"}),
        ),
        "path_not_permitted",
    );
    assert!(
        !tmp.path()
            .join("Agents/jarvis.tony/topics/.hidden.jarvis.tony.md")
            .exists()
    );
}

// --- edit_memory_note ---

#[test]
fn edit_unique_succeeds() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","content":"alpha beta gamma"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "edit_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","search_string":"beta","replace_string":"BETA"}),
    ));
    assert_eq!(body["chars_replaced"], 4);
    let read = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md"}),
    ));
    assert_eq!(read["content"], "alpha BETA gamma");
}

#[test]
fn edit_read_only_region_is_denied() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "alpha beta");
    assert_code(
        call(
            &tb,
            "edit_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Actions/release.md","search_string":"beta","replace_string":"x"}),
        ),
        "write_denied",
    );
}

#[test]
fn edit_missing_search_is_not_found() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","content":"alpha"}),
    )
    .unwrap();
    assert_code(
        call(
            &tb,
            "edit_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","search_string":"zeta","replace_string":"x"}),
        ),
        "edit_search_not_found",
    );
}

#[test]
fn edit_ambiguous_search_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","content":"dup dup"}),
    )
    .unwrap();
    assert_code(
        call(
            &tb,
            "edit_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","search_string":"dup","replace_string":"x"}),
        ),
        "edit_search_ambiguous",
    );
}

// --- delete_memory_note ---

#[test]
fn delete_own_scope_succeeds() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","content":"x"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "delete_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md"}),
    ));
    assert_eq!(body["deleted"], true);
}

#[test]
fn delete_under_readonly_is_denied() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Readonly);
    assert_code(
        call(
            &tb,
            "delete_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md"}),
        ),
        "write_denied",
    );
}

#[test]
fn delete_outside_under_namespaced_is_denied() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "x");
    assert_code(
        call(
            &tb,
            "delete_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Actions/release.md"}),
        ),
        "write_denied",
    );
}

#[test]
fn delete_outside_under_scoped_is_path_not_permitted() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Scoped);
    write_outside(&tmp, "Actions/release.md", "x");
    assert_code(
        call(
            &tb,
            "delete_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Actions/release.md"}),
        ),
        "path_not_permitted",
    );
}

#[test]
fn delete_missing_is_not_found() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "delete_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/nope.md"}),
        ),
        "not_found",
    );
}

#[test]
fn delete_other_scope_is_unreachable() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    // sam owns the file; tony's delete of the same logical name must miss it.
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"sam","path":"Agents/topics/n.md","content":"sam"}),
    )
    .unwrap();
    assert_code(
        call(
            &tb,
            "delete_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md"}),
        ),
        "not_found",
    );
    assert!(
        tmp.path()
            .join("Agents/jarvis.sam/topics/n.jarvis.sam.md")
            .exists()
    );
}

// --- wrapper-only root files ---

#[test]
fn write_root_core_file_is_rejected_naming_wrapper() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for f in ["MEMORY.md", "USER.md", "PERSONA.md"] {
        let res = call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":format!("Agents/{f}"),"content":"x"}),
        );
        match res {
            Err(e) => {
                assert_eq!(e.code().as_str(), "path_not_permitted", "for {f}");
                assert!(
                    e.to_string().contains("evolve_core_persona"),
                    "message should name the wrapper for {f}: {e}"
                );
            }
            Ok(_) => panic!("expected rejection writing root {f}"),
        }
        // File was never created.
        assert!(
            !tmp.path()
                .join(format!(
                    "Agents/jarvis.tony/{}",
                    f.replace(".md", ".jarvis.tony.md")
                ))
                .exists()
        );
    }
}

#[test]
fn write_root_heartbeat_names_heartbeat_wrapper() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let res = call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/HEARTBEAT.md","content":"x"}),
    );
    match res {
        Err(e) => {
            assert_eq!(e.code().as_str(), "path_not_permitted");
            assert!(e.to_string().contains("update_task_heartbeat"), "got: {e}");
        }
        Ok(_) => panic!("expected rejection"),
    }
}

#[test]
fn edit_root_core_file_is_rejected_and_unchanged() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    // Seed MEMORY.md through the wrapper so a file exists on disk.
    call(
        &tb,
        "evolve_core_persona",
        json!({"agent":"jarvis","user":"tony","which":"memory","content":"alpha beta"}),
    )
    .unwrap();
    assert_code(
        call(
            &tb,
            "edit_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/MEMORY.md","search_string":"beta","replace_string":"BETA"}),
        ),
        "path_not_permitted",
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("Agents/jarvis.tony/MEMORY.jarvis.tony.md"))
            .unwrap(),
        "alpha beta"
    );
}

#[test]
fn delete_root_core_file_is_rejected_and_unchanged() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "evolve_core_persona",
        json!({"agent":"jarvis","user":"tony","which":"persona","content":"soul"}),
    )
    .unwrap();
    assert_code(
        call(
            &tb,
            "delete_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/PERSONA.md"}),
        ),
        "path_not_permitted",
    );
    assert!(
        tmp.path()
            .join("Agents/jarvis.tony/PERSONA.jarvis.tony.md")
            .exists()
    );
}

#[test]
fn subfolder_write_edit_delete_inside_agents_are_allowed() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    // Write under a subfolder succeeds.
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/auth/jwt.md","content":"alpha beta"}),
    )
    .unwrap();
    // Edit succeeds.
    call(
        &tb,
        "edit_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/auth/jwt.md","search_string":"beta","replace_string":"BETA"}),
    )
    .unwrap();
    // Delete succeeds.
    let body = structured(call(
        &tb,
        "delete_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/auth/jwt.md"}),
    ));
    assert_eq!(body["deleted"], true);
}

#[test]
fn outside_region_policy_behavior_is_unaffected_by_root_rule() {
    let tmp = TempDir::new().unwrap();
    // Under namespaced policy, writes outside the agents folder stay write_denied
    // (the root rule only governs inside the agents folder).
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "orig");
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Actions/release.md","content":"new"}),
        ),
        "write_denied",
    );
}

// --- load_session_context ---

#[test]
fn load_session_context_all_present() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for which in ["persona", "prompt", "rules", "user", "memory"] {
        call(
            &tb,
            "evolve_core_persona",
            json!({"agent":"jarvis","user":"tony","which":which,"content":format!("BODY-{which}")}),
        )
        .unwrap();
    }
    let body = structured(call(
        &tb,
        "load_session_context",
        json!({"agent":"jarvis","user":"tony"}),
    ));
    let rendered = body["rendered"].as_str().unwrap();
    // Each foundational file's contents are woven into the rendered output.
    assert!(rendered.contains("BODY-persona"));
    assert!(rendered.contains("BODY-memory"));
    assert_eq!(body["missing"].as_array().unwrap().len(), 0);
}

#[test]
fn load_session_context_some_missing() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "evolve_core_persona",
        json!({"agent":"jarvis","user":"tony","which":"persona","content":"p"}),
    )
    .unwrap();
    call(
        &tb,
        "evolve_core_persona",
        json!({"agent":"jarvis","user":"tony","which":"rules","content":"r"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "load_session_context",
        json!({"agent":"jarvis","user":"tony"}),
    ));
    let rendered = body["rendered"].as_str().unwrap();
    // Present file is substituted; absent ones show the missing sentinel.
    assert!(rendered.contains('p'));
    assert!(rendered.contains("(not yet recorded"));
    let missing: Vec<&str> = body["missing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        missing.contains(&"PROMPT.md")
            && missing.contains(&"USER.md")
            && missing.contains(&"MEMORY.md")
    );
}

#[test]
fn load_session_context_rejects_extra_args() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "load_session_context",
            json!({"agent":"jarvis","user":"tony","path":"x"}),
        ),
        "invalid_argument",
    );
    assert_code(
        call(
            &tb,
            "load_session_context",
            json!({"agent":"jarvis","user":"tony","which":"persona"}),
        ),
        "invalid_argument",
    );
}

// --- evolve_core_persona ---

#[test]
fn evolve_writes_each_foundational_file() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for (which, file) in [
        ("persona", "PERSONA.md"),
        ("prompt", "PROMPT.md"),
        ("rules", "RULES.md"),
        ("user", "USER.md"),
        ("memory", "MEMORY.md"),
    ] {
        call(
            &tb,
            "evolve_core_persona",
            json!({"agent":"jarvis","user":"tony","which":which,"content":which}),
        )
        .unwrap();
        let physical = tmp.path().join(format!(
            "Agents/jarvis.tony/{}",
            file.replace(".md", ".jarvis.tony.md")
        ));
        assert_eq!(std::fs::read_to_string(physical).unwrap(), which);
    }
}

#[test]
fn evolve_invalid_which_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "evolve_core_persona",
            json!({"agent":"jarvis","user":"tony","which":"bogus","content":"x"}),
        ),
        "invalid_argument",
    );
}

#[test]
fn evolve_rejects_path_arg() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "evolve_core_persona",
            json!({"agent":"jarvis","user":"tony","which":"persona","content":"x","path":"y"}),
        ),
        "invalid_argument",
    );
}

#[test]
fn evolve_under_readonly_is_denied() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Readonly);
    assert_code(
        call(
            &tb,
            "evolve_core_persona",
            json!({"agent":"jarvis","user":"tony","which":"persona","content":"x"}),
        ),
        "write_denied",
    );
}

#[test]
fn evolve_user_within_cap_succeeds() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let content = "line\n".repeat(100);
    call(
        &tb,
        "evolve_core_persona",
        json!({"agent":"jarvis","user":"tony","which":"user","content":content}),
    )
    .unwrap();
}

#[test]
fn evolve_user_over_cap_is_rejected_and_unchanged() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let content = "line\n".repeat(101);
    assert_code(
        call(
            &tb,
            "evolve_core_persona",
            json!({"agent":"jarvis","user":"tony","which":"user","content":content}),
        ),
        "invalid_argument",
    );
    assert!(
        !tmp.path()
            .join("Agents/jarvis.tony/USER.jarvis.tony.md")
            .exists()
    );
}

#[test]
fn evolve_memory_within_cap_succeeds() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let content = "line\n".repeat(200);
    call(
        &tb,
        "evolve_core_persona",
        json!({"agent":"jarvis","user":"tony","which":"memory","content":content}),
    )
    .unwrap();
}

#[test]
fn evolve_memory_over_cap_is_rejected_and_unchanged() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let content = "line\n".repeat(201);
    assert_code(
        call(
            &tb,
            "evolve_core_persona",
            json!({"agent":"jarvis","user":"tony","which":"memory","content":content}),
        ),
        "invalid_argument",
    );
    assert!(
        !tmp.path()
            .join("Agents/jarvis.tony/MEMORY.jarvis.tony.md")
            .exists()
    );
}

// --- update_task_heartbeat ---

#[test]
fn heartbeat_writes_state_file() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "update_task_heartbeat",
        json!({"agent":"jarvis","user":"tony","content":"working"}),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(
            tmp.path()
                .join("Agents/jarvis.tony/HEARTBEAT.jarvis.tony.md")
        )
        .unwrap(),
        "working"
    );
}

// --- append_diary_entry ---

#[test]
fn diary_creates_then_appends() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "append_diary_entry",
        json!({"agent":"jarvis","user":"tony","content":"first"}),
    )
    .unwrap();
    call(
        &tb,
        "append_diary_entry",
        json!({"agent":"jarvis","user":"tony","content":"second"}),
    )
    .unwrap();
    // Find the single diary file under the scope dir.
    let diary_dir = tmp.path().join("Agents/jarvis.tony/diary");
    let file = std::fs::read_dir(&diary_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let contents = std::fs::read_to_string(file).unwrap();
    // A newly created diary file opens with a `# <YYYY-MM-DD>` H1.
    assert!(contents.starts_with("# "));
    let first_line = contents.lines().next().unwrap();
    assert_eq!(first_line.len(), "# 2026-01-01".len());
    assert!(contents.contains("first"));
    assert!(contents.contains("second"));
    // Two timestamped sections beneath the H1.
    assert_eq!(contents.matches("\n## ").count(), 2);
}

#[test]
fn diary_entry_with_title_uses_em_dash_heading() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "append_diary_entry",
        json!({"agent":"jarvis","user":"tony","content":"body","title":"Task pickup"}),
    )
    .unwrap();
    let diary_dir = tmp.path().join("Agents/jarvis.tony/diary");
    let file = std::fs::read_dir(&diary_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let contents = std::fs::read_to_string(file).unwrap();
    assert!(contents.contains(" — Task pickup\n"));
}

#[test]
fn diary_entry_without_title_uses_bare_time_heading() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "append_diary_entry",
        json!({"agent":"jarvis","user":"tony","content":"body"}),
    )
    .unwrap();
    let diary_dir = tmp.path().join("Agents/jarvis.tony/diary");
    let file = std::fs::read_dir(&diary_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let contents = std::fs::read_to_string(file).unwrap();
    // No em-dash title separator anywhere; the heading is just `## HH:MM:SS`.
    assert!(!contents.contains(" — "));
}

#[test]
fn diary_concurrent_appends_are_serialised() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    std::thread::scope(|s| {
        for n in 0..8 {
            let tb = &tb;
            s.spawn(move || {
                call(
                    tb,
                    "append_diary_entry",
                    json!({"agent":"jarvis","user":"tony","content":format!("entry-{n}")}),
                )
                .unwrap();
            });
        }
    });
    let diary_dir = tmp.path().join("Agents/jarvis.tony/diary");
    let file = std::fs::read_dir(&diary_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let contents = std::fs::read_to_string(file).unwrap();
    // All eight entries survive; the per-target lock prevents lost updates.
    for n in 0..8 {
        assert!(
            contents.contains(&format!("entry-{n}")),
            "missing entry-{n}"
        );
    }
}

#[test]
fn diary_rejects_path_arg() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "append_diary_entry",
            json!({"agent":"jarvis","user":"tony","content":"x","path":"y"}),
        ),
        "invalid_argument",
    );
}

#[test]
fn diary_empty_content_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "append_diary_entry",
            json!({"agent":"jarvis","user":"tony","content":""}),
        ),
        "invalid_argument",
    );
}

// --- common tool input contract ---

#[test]
fn missing_scope_key_is_reported_by_name() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    match call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","path":"Agents/topics/n.md"}),
    ) {
        Err(AgentmemError::MissingScope { key }) => assert_eq!(key, "user"),
        other => panic!("expected missing_scope(user), got {other:?}"),
    }
}

#[test]
fn unexpected_scope_param_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>", Policy::Namespaced);
    assert_code(
        call(
            &tb,
            "read_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md"}),
        ),
        "invalid_argument",
    );
}

#[test]
fn custom_scheme_keys_are_honoured() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(
        &tmp,
        "Agents",
        "<team>.<agent>.<env>.<user>",
        Policy::Namespaced,
    );
    call(&tb, "write_memory_note", json!({"team":"platform","agent":"jarvis","env":"prod","user":"tony","path":"Agents/topics/plan.md","content":"x"})).unwrap();
    assert!(
        tmp.path()
            .join("Agents/platform.jarvis.prod.tony/topics/plan.platform.jarvis.prod.tony.md")
            .exists()
    );
}

#[test]
fn empty_scheme_requires_no_scope_args() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "", Policy::Namespaced);
    call(
        &tb,
        "write_memory_note",
        json!({"path":"Agents/topics/n.md","content":"x"}),
    )
    .unwrap();
    assert!(tmp.path().join("Agents/topics/n.md").exists());
    // Supplying a scope field is rejected.
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","path":"Agents/topics/n.md","content":"x"}),
        ),
        "invalid_argument",
    );
}

// --- recall_memory_notes ---

#[test]
fn recall_finds_a_written_note_by_content() {
    let tmp = TempDir::new().unwrap();
    let tb = recall_toolbox(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"the borrow checker enforces ownership"}),
    )
    .unwrap();
    let out = structured(call(
        &tb,
        "recall_memory_notes",
        json!({"agent":"jarvis","user":"tony","query":"borrow"}),
    ));
    let hits = out["hits"].as_array().unwrap();
    assert!(hits.iter().any(|h| h["path"] == "Agents/topics/rust.md"));
    let top = &hits[0];
    assert!(top["score"].as_f64().unwrap() > 0.0 && top["score"].as_f64().unwrap() <= 1.0);
    assert!(!top["snippets"].as_array().unwrap().is_empty());
}

#[test]
fn recall_does_not_cross_scope_boundaries() {
    let tmp = TempDir::new().unwrap();
    let tb = recall_toolbox(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/a.md","content":"shared keyword zebra"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"sam","path":"Agents/topics/b.md","content":"shared keyword zebra"}),
    )
    .unwrap();
    let out = structured(call(
        &tb,
        "recall_memory_notes",
        json!({"agent":"jarvis","user":"tony","query":"zebra"}),
    ));
    let hits = out["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["path"], "Agents/topics/a.md");
}

#[test]
fn recall_requires_a_query_regex_or_filter() {
    let tmp = TempDir::new().unwrap();
    let tb = recall_toolbox(&tmp);
    assert_code(
        call(
            &tb,
            "recall_memory_notes",
            json!({"agent":"jarvis","user":"tony"}),
        ),
        "invalid_argument",
    );
}

#[test]
fn recall_property_filters_are_unsupported_on_simple_backend() {
    let tmp = TempDir::new().unwrap();
    let tb = recall_toolbox(&tmp);
    assert_code(
        call(
            &tb,
            "recall_memory_notes",
            json!({
                "agent":"jarvis","user":"tony",
                "filters":[{"key":"tag","op":"eq","value":"rust"}]
            }),
        ),
        "unsupported",
    );
}

#[test]
fn recall_tool_absent_when_backend_off() {
    // The default `toolbox` helper builds with recall disabled (None).
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert!(
        tb.list_tools()
            .iter()
            .all(|t| t.name != "recall_memory_notes")
    );
}
