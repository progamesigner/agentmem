//! In-process tool-handler tests covering every scenario in
//! `specs/memory-tools/spec.md` (task 8.12).
//!
//! The tests drive [`Toolbox`] directly — the same code path the MCP `call_tool`
//! handler uses — so each scenario exercises scope extraction, path resolution,
//! policy gating, visibility filtering, and storage end to end.

use std::sync::Arc;
use std::time::Duration;

use agentmem::AgentmemError;
use agentmem::config::{Grant, RecallBackendKind, RecallConfig};
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
    recall_toolbox_tz(tmp, Tz::UTC)
}

/// A recall toolbox with a configurable timezone (interprets date-only time bounds).
fn recall_toolbox_tz(tmp: &TempDir, timezone: Tz) -> Toolbox {
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
        timezone,
        tmp.path().join("AGENT_SESSION_CONTEXT.md"),
        recall,
    )
}

/// A recall toolbox whose index never goes stale on its own (hour-long
/// freshness and debounce, no watcher started): after the first build, only the
/// server's own `recall_on_write` notifications can update it.
fn frozen_recall_toolbox(tmp: &TempDir) -> Toolbox {
    frozen_toolbox(tmp, RecallBackendKind::Simple)
}

/// [`frozen_recall_toolbox`] with a configurable backend.
fn frozen_toolbox(tmp: &TempDir, backend: RecallBackendKind) -> Toolbox {
    let mk = || {
        PathResolver::new(
            tmp.path().canonicalize().unwrap(),
            Utf8PathBuf::from("Agents"),
            Scheme::parse("<agent>.<user>").unwrap(),
        )
    };
    let storage = Storage::new(mk(), true, false, &[]);
    let config = RecallConfig {
        backend,
        watch_debounce: Duration::from_secs(3600),
        regex_scan_byte_cap: usize::MAX,
        max_resident_scopes: 256,
        freshness: Duration::from_secs(3600),
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
    tb.call(name, &obj, &Grant::AllScopes)
        .expect("tool name must be known")
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
fn list_glob_filters_by_virtual_path() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/diary/2026-06-10.md","content":"x"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"x"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","glob":"Agents/diary/2026-*"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items, vec!["Agents/diary/2026-06-10.md"]);
}

#[test]
fn list_glob_composes_with_path_prefix() {
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
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/notes.txt","content":"x"}),
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
        json!({"agent":"jarvis","user":"tony","path_prefix":"topics","glob":"**/*.md"}),
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
fn list_invalid_glob_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "list_memory_notes",
            json!({"agent":"jarvis","user":"tony","glob":"Agents/[unterminated"}),
        ),
        "invalid_argument",
    );
}

#[test]
fn list_glob_preserves_ordering_and_pagination() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for name in ["c", "a", "b"] {
        call(&tb, "write_memory_note", json!({"agent":"jarvis","user":"tony","path":format!("Agents/topics/{name}.md"),"content":"x"})).unwrap();
    }
    // A non-matching note that the glob must exclude from every page.
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/skip.txt","content":"x"}),
    )
    .unwrap();

    let page1 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","glob":"Agents/topics/*.md","limit":2}),
    ));
    let items1: Vec<&str> = page1["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items1, vec!["Agents/topics/a.md", "Agents/topics/b.md"]);
    let cursor = page1["next_cursor"]
        .as_str()
        .expect("cursor on first page")
        .to_string();

    let page2 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","glob":"Agents/topics/*.md","limit":2,"cursor":cursor}),
    ));
    let items2: Vec<&str> = page2["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items2, vec!["Agents/topics/c.md"]);
    assert!(page2["next_cursor"].is_null());
}

#[test]
fn list_default_order_is_ascending() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for name in ["c", "a", "b"] {
        call(&tb, "write_memory_note", json!({"agent":"jarvis","user":"tony","path":format!("Agents/topics/{name}.md"),"content":"x"})).unwrap();
    }
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
    assert_eq!(
        items,
        vec![
            "Agents/topics/a.md",
            "Agents/topics/b.md",
            "Agents/topics/c.md"
        ]
    );
}

#[test]
fn list_name_desc_returns_descending_order() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for name in ["2026-01-01", "2026-06-10"] {
        call(&tb, "write_memory_note", json!({"agent":"jarvis","user":"tony","path":format!("Agents/diary/{name}.md"),"content":"x"})).unwrap();
    }
    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","order":"name_desc"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        items,
        vec!["Agents/diary/2026-06-10.md", "Agents/diary/2026-01-01.md"]
    );
}

#[test]
fn list_invalid_order_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "list_memory_notes",
            json!({"agent":"jarvis","user":"tony","order":"recency_desc"}),
        ),
        "invalid_argument",
    );
}

#[test]
fn list_name_desc_orders_before_pagination() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for name in ["a", "b", "c"] {
        call(&tb, "write_memory_note", json!({"agent":"jarvis","user":"tony","path":format!("Agents/topics/{name}.md"),"content":"x"})).unwrap();
    }
    let page1 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","order":"name_desc","limit":2}),
    ));
    let items1: Vec<&str> = page1["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items1, vec!["Agents/topics/c.md", "Agents/topics/b.md"]);
    let cursor = page1["next_cursor"]
        .as_str()
        .expect("cursor on first page")
        .to_string();

    let page2 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","order":"name_desc","limit":2,"cursor":cursor}),
    ));
    let items2: Vec<&str> = page2["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items2, vec!["Agents/topics/a.md"]);
    assert!(page2["next_cursor"].is_null());
}

#[test]
fn list_default_view_returns_files() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"x"}),
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
    assert_eq!(items, vec!["Agents/topics/rust.md"]);
}

#[test]
fn list_dirs_view_returns_distinct_directories() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for path in [
        "Agents/diary/2026-06-10.md",
        "Agents/topics/rust.md",
        "Agents/topics/python.md",
    ] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":path,"content":"x"}),
        )
        .unwrap();
    }

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","view":"dirs"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items, vec!["Agents", "Agents/diary", "Agents/topics"]);
}

#[test]
fn list_dirs_view_honors_path_prefix() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for path in [
        "Agents/topics/sub/rust.md",
        "Agents/topics/python.md",
        "Agents/diary/2026-06-10.md",
    ] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":path,"content":"x"}),
        )
        .unwrap();
    }

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","view":"dirs","path_prefix":"topics"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items, vec!["Agents", "Agents/topics", "Agents/topics/sub"]);
}

#[test]
fn list_invalid_view_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "list_memory_notes",
            json!({"agent":"jarvis","user":"tony","view":"tree"}),
        ),
        "invalid_argument",
    );
}

#[test]
fn list_dirs_view_paginates() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for path in ["Agents/a/x.md", "Agents/b/x.md", "Agents/c/x.md"] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":path,"content":"x"}),
        )
        .unwrap();
    }

    // Directory set is {Agents, Agents/a, Agents/b, Agents/c}; page by 2.
    let page1 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","view":"dirs","limit":2}),
    ));
    let items1: Vec<&str> = page1["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items1, vec!["Agents", "Agents/a"]);
    let cursor = page1["next_cursor"]
        .as_str()
        .expect("cursor on first page")
        .to_string();

    let page2 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"jarvis","user":"tony","view":"dirs","limit":2,"cursor":cursor}),
    ));
    let items2: Vec<&str> = page2["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items2, vec!["Agents/b", "Agents/c"]);
    assert!(page2["next_cursor"].is_null());
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

// --- read_memory_note backlinks ---

#[test]
fn read_backlinks_for_wikilink_and_markdown_referrers() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for (path, content) in [
        ("Agents/topics/rust.md", "the target"),
        ("Agents/diary/2026-06-10.md", "worked on [[rust]] today"),
        (
            "Agents/notes/memo.md",
            "see [the Rust note](topics/rust.md)",
        ),
        ("Agents/notes/unrelated.md", "no links"),
    ] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":path,"content":content}),
        )
        .unwrap();
    }

    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","backlinks":true}),
    ));
    assert_eq!(body["content"], "the target");
    assert_eq!(
        body["backlinks"],
        json!(["Agents/diary/2026-06-10.md", "Agents/notes/memo.md"])
    );
}

#[test]
fn read_backlinks_lists_a_referring_note_once() {
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
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/memo.md",
               "content":"[[rust]], again [[rust|aliased]], embedded ![[rust]]"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","backlinks":true}),
    ));
    assert_eq!(body["backlinks"], json!(["Agents/notes/memo.md"]));
}

#[test]
fn read_backlinks_ordering_is_deterministic() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"x"}),
    )
    .unwrap();
    for name in ["c", "a", "b"] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":format!("Agents/notes/{name}.md"),"content":"[[rust]]"}),
        )
        .unwrap();
    }

    let args =
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","backlinks":true});
    let first = structured(call(&tb, "read_memory_note", args.clone()));
    assert_eq!(
        first["backlinks"],
        json!([
            "Agents/notes/a.md",
            "Agents/notes/b.md",
            "Agents/notes/c.md"
        ])
    );
    let second = structured(call(&tb, "read_memory_note", args));
    assert_eq!(first["backlinks"], second["backlinks"]);
}

#[test]
fn read_backlinks_never_scans_other_scopes() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "shared target");
    // Another scope links to the shared note; the caller's own note does too.
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"sam","path":"Agents/notes/sam.md","content":"[[release]]"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/tony.md","content":"[[release]]"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Actions/release.md","backlinks":true}),
    ));
    assert_eq!(body["backlinks"], json!(["Agents/notes/tony.md"]));
}

#[test]
fn read_backlinks_scoped_policy_excludes_shared_referrers() {
    let tmp = TempDir::new().unwrap();
    // Shared note links to a name only resolvable as the caller's own note.
    write_outside(&tmp, "Actions/pointer.md", "[[rust]]");
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Scoped);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"x"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/memo.md","content":"[[rust]]"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","backlinks":true}),
    ));
    assert_eq!(body["backlinks"], json!(["Agents/notes/memo.md"]));
}

#[test]
fn read_without_backlinks_flag_is_unchanged() {
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
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/memo.md","content":"[[rust]]"}),
    )
    .unwrap();

    let absent = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md"}),
    ));
    assert_eq!(absent, json!({"content": "x"}));

    let explicit_false = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","backlinks":false}),
    ));
    assert_eq!(explicit_false, json!({"content": "x"}));
}

// --- read_memory_note line ranges ---

/// A note of `lines` numbered lines: `l1\n` through `l<lines>\n`.
fn numbered_note(lines: usize) -> String {
    (1..=lines).map(|i| format!("l{i}\n")).collect()
}

/// The expected slice of [`numbered_note`] from line `from` through `to`.
fn numbered_lines(from: usize, to: usize) -> String {
    (from..=to).map(|i| format!("l{i}\n")).collect()
}

#[test]
fn read_mid_file_range_returns_slice_and_total_lines() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/long.md","content":numbered_note(50)}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/long.md","offset":11,"limit":10}),
    ));
    assert_eq!(body["content"], numbered_lines(11, 20));
    assert_eq!(body["total_lines"], 50);
}

#[test]
fn read_offset_alone_reads_to_the_end() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/long.md","content":numbered_note(50)}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/long.md","offset":41}),
    ));
    assert_eq!(body["content"], numbered_lines(41, 50));
    assert_eq!(body["total_lines"], 50);
}

#[test]
fn read_limit_alone_reads_from_the_start() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/long.md","content":numbered_note(50)}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/long.md","limit":5}),
    ));
    assert_eq!(body["content"], numbered_lines(1, 5));
    assert_eq!(body["total_lines"], 50);
}

#[test]
fn read_offset_past_eof_is_empty_not_an_error() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/short.md","content":numbered_note(10)}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/short.md","offset":11}),
    ));
    assert_eq!(body["content"], "");
    assert_eq!(body["total_lines"], 10);
}

#[test]
fn read_zero_offset_or_limit_is_invalid_argument() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/note.md","content":"x"}),
    )
    .unwrap();
    for args in [
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/note.md","offset":0}),
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/note.md","limit":0}),
    ] {
        assert_code(call(&tb, "read_memory_note", args), "invalid_argument");
    }
}

#[test]
fn read_without_range_carries_no_total_lines() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/note.md","content":"a\nb\n"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/note.md"}),
    ));
    assert_eq!(body, json!({"content": "a\nb\n"}));
}

#[test]
fn read_range_composes_with_backlinks() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"a\nb\nc\n"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/memo.md","content":"[[rust]]"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md",
               "offset":2,"limit":1,"backlinks":true}),
    ));
    assert_eq!(body["content"], "b\n");
    assert_eq!(body["total_lines"], 3);
    assert_eq!(body["backlinks"], json!(["Agents/notes/memo.md"]));
}

#[test]
fn read_range_slices_the_link_stripped_view() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    // The link target must exist for [[rust]] to resolve, expand on write, and
    // be stored in the suffixed on-disk form.
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"seed"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/memo.md",
               "content":"one\ntwo\nsee [[rust]]\nfour"}),
    )
    .unwrap();
    let stored = std::fs::read_to_string(
        tmp.path()
            .join("Agents/jarvis.tony/notes/memo.jarvis.tony.md"),
    )
    .unwrap();
    assert!(stored.contains("[[rust.jarvis.tony]]"), "stored: {stored}");

    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/memo.md","offset":3,"limit":1}),
    ));
    assert_eq!(body["content"], "see [[rust]]\n");
    assert_eq!(body["total_lines"], 4);
    // Line numbers match a whole-note read.
    let whole = read_clean(&tb, "Agents/notes/memo.md");
    assert_eq!(
        whole.split_inclusive('\n').nth(2).unwrap(),
        body["content"].as_str().unwrap()
    );
}

// --- read_memory_notes ---

#[test]
fn batch_read_returns_contents_in_request_order() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"own"}),
    )
    .unwrap();
    write_outside(&tmp, "Actions/release.md", "shared");

    let body = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony",
               "paths":["Agents/topics/rust.md","Actions/release.md"]}),
    ));
    assert_eq!(
        body["notes"],
        json!([
            {"path": "Agents/topics/rust.md", "content": "own"},
            {"path": "Actions/release.md", "content": "shared"},
        ])
    );
}

#[test]
fn batch_read_strips_links_like_single_read() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    // The link target must exist for [[rust]] to resolve, expand on write, and
    // be stored in the suffixed on-disk form.
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"seed"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/memo.md",
               "content":"see [[rust]]"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony","paths":["Agents/notes/memo.md"]}),
    ));
    let batch_content = body["notes"][0]["content"].as_str().unwrap();
    assert_eq!(batch_content, "see [[rust]]");
    assert_eq!(batch_content, read_clean(&tb, "Agents/notes/memo.md"));
}

#[test]
fn batch_read_duplicate_paths_answered_positionally() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"x"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony",
               "paths":["Agents/topics/rust.md","Agents/topics/rust.md"]}),
    ));
    assert_eq!(
        body["notes"],
        json!([
            {"path": "Agents/topics/rust.md", "content": "x"},
            {"path": "Agents/topics/rust.md", "content": "x"},
        ])
    );
}

#[test]
fn batch_read_partial_failure_does_not_void_the_batch() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for (path, content) in [
        ("Agents/topics/a.md", "first"),
        ("Agents/topics/c.md", "third"),
    ] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":path,"content":content}),
        )
        .unwrap();
    }

    let body = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony",
               "paths":["Agents/topics/a.md","Agents/topics/missing.md","Agents/topics/c.md"]}),
    ));
    let notes = body["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 3);
    assert_eq!(
        notes[0],
        json!({"path": "Agents/topics/a.md", "content": "first"})
    );
    assert_eq!(notes[1]["path"], "Agents/topics/missing.md");
    assert_eq!(notes[1]["error"]["code"], "not_found");
    assert!(notes[1].get("content").is_none());
    assert_eq!(
        notes[2],
        json!({"path": "Agents/topics/c.md", "content": "third"})
    );
}

#[test]
fn batch_read_hidden_entry_is_path_not_permitted() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/ok.md","content":"fine"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony",
               "paths":["Agents/topics/.secret.md","Agents/topics/ok.md"]}),
    ));
    let notes = body["notes"].as_array().unwrap();
    assert_eq!(notes[0]["error"]["code"], "path_not_permitted");
    assert_eq!(
        notes[1],
        json!({"path": "Agents/topics/ok.md", "content": "fine"})
    );
}

#[test]
fn batch_read_policy_denied_entry_is_path_not_permitted() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Scoped);
    write_outside(&tmp, "Actions/release.md", "notes");
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/ok.md","content":"fine"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony",
               "paths":["Actions/release.md","Agents/topics/ok.md"]}),
    ));
    let notes = body["notes"].as_array().unwrap();
    assert_eq!(notes[0]["error"]["code"], "path_not_permitted");
    assert_eq!(
        notes[1],
        json!({"path": "Agents/topics/ok.md", "content": "fine"})
    );
}

#[test]
fn batch_read_empty_array_is_invalid_argument() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "read_memory_notes",
            json!({"agent":"jarvis","user":"tony","paths":[]}),
        ),
        "invalid_argument",
    );
}

#[test]
fn batch_read_over_twenty_entries_is_invalid_argument() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let paths: Vec<String> = (0..21).map(|i| format!("Agents/topics/{i}.md")).collect();
    assert_code(
        call(
            &tb,
            "read_memory_notes",
            json!({"agent":"jarvis","user":"tony","paths":paths}),
        ),
        "invalid_argument",
    );
}

#[test]
fn batch_read_non_string_entry_is_invalid_argument() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "read_memory_notes",
            json!({"agent":"jarvis","user":"tony","paths":["Agents/topics/ok.md", 7]}),
        ),
        "invalid_argument",
    );
}

#[test]
fn batch_read_mixed_string_and_ranged_entries() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"own"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/diary/2026-06-10.md","content":numbered_note(10)}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony",
               "paths":["Agents/topics/rust.md",
                        {"path":"Agents/diary/2026-06-10.md","offset":1,"limit":5}]}),
    ));
    assert_eq!(
        body["notes"][0],
        json!({"path": "Agents/topics/rust.md", "content": "own"})
    );
    assert_eq!(
        body["notes"][1],
        json!({"path": "Agents/diary/2026-06-10.md",
               "content": numbered_lines(1, 5), "total_lines": 10})
    );
}

#[test]
fn batch_read_ranged_entry_past_eof_succeeds() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/short.md","content":numbered_note(3)}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/other.md","content":"fine"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony",
               "paths":[{"path":"Agents/topics/short.md","offset":9},
                        "Agents/topics/other.md"]}),
    ));
    assert_eq!(
        body["notes"][0],
        json!({"path": "Agents/topics/short.md", "content": "", "total_lines": 3})
    );
    assert_eq!(
        body["notes"][1],
        json!({"path": "Agents/topics/other.md", "content": "fine"})
    );
}

#[test]
fn batch_read_malformed_entry_rejects_the_whole_call() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/ok.md","content":"fine"}),
    )
    .unwrap();
    for entry in [
        json!({"offset": 3}),
        json!({"path": "Agents/topics/ok.md", "offset": 0}),
        json!({"path": "Agents/topics/ok.md", "limit": 0}),
        json!({"path": "Agents/topics/ok.md", "unexpected": true}),
        json!({"path": 7}),
    ] {
        assert_code(
            call(
                &tb,
                "read_memory_notes",
                json!({"agent":"jarvis","user":"tony","paths":["Agents/topics/ok.md", entry]}),
            ),
            "invalid_argument",
        );
    }
}

#[test]
fn batch_read_range_parity_with_single_read() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/long.md","content":numbered_note(50)}),
    )
    .unwrap();

    let single = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/long.md","offset":11,"limit":10}),
    ));
    let batch = structured(call(
        &tb,
        "read_memory_notes",
        json!({"agent":"jarvis","user":"tony",
               "paths":[{"path":"Agents/topics/long.md","offset":11,"limit":10}]}),
    ));
    assert_eq!(batch["notes"][0]["content"], single["content"]);
    assert_eq!(batch["notes"][0]["total_lines"], single["total_lines"]);
}

// --- rename_memory_note ---

/// Read a note's clean content via the tool.
fn read_clean(tb: &Toolbox, path: &str) -> String {
    structured(call(
        tb,
        "read_memory_note",
        json!({"agent":"jarvis","user":"tony","path":path}),
    ))["content"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn rename_rewrites_wikilink_and_markdown_referrers() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for (path, content) in [
        ("Agents/topics/rust.md", "the rust note"),
        (
            "Agents/diary/2026-06-10.md",
            "worked on [[rust]], [[rust#install|the note]], and ![[rust]]",
        ),
        (
            "Agents/notes/memo.md",
            "see [the Rust note](topics/rust.md)",
        ),
        ("Agents/notes/unrelated.md", "no links"),
    ] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":path,"content":content}),
        )
        .unwrap();
    }

    let body = structured(call(
        &tb,
        "rename_memory_note",
        json!({"agent":"jarvis","user":"tony",
               "path":"Agents/topics/rust.md","new_path":"Agents/topics/rust-lang.md"}),
    ));
    assert_eq!(
        body,
        json!({
            "renamed": true,
            "path": "Agents/topics/rust.md",
            "new_path": "Agents/topics/rust-lang.md",
            "notes_rewritten": 2,
        })
    );

    // The destination carries the content; the source is gone.
    assert_eq!(
        read_clean(&tb, "Agents/topics/rust-lang.md"),
        "the rust note"
    );
    assert_code(
        call(
            &tb,
            "read_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md"}),
        ),
        "not_found",
    );

    // Referring notes round-trip to the clean new target, decorations preserved.
    assert_eq!(
        read_clean(&tb, "Agents/diary/2026-06-10.md"),
        "worked on [[rust-lang]], [[rust-lang#install|the note]], and ![[rust-lang]]"
    );
    assert_eq!(
        read_clean(&tb, "Agents/notes/memo.md"),
        "see [the Rust note](topics/rust-lang.md)"
    );
    assert_eq!(read_clean(&tb, "Agents/notes/unrelated.md"), "no links");

    // On disk the rewritten links carry the suffixed/physical forms.
    let diary_raw = std::fs::read_to_string(
        tmp.path()
            .join("Agents/jarvis.tony/diary/2026-06-10.jarvis.tony.md"),
    )
    .unwrap();
    assert!(diary_raw.contains("[[rust-lang.jarvis.tony]]"));
    let memo_raw = std::fs::read_to_string(
        tmp.path()
            .join("Agents/jarvis.tony/notes/memo.jarvis.tony.md"),
    )
    .unwrap();
    assert!(memo_raw.contains("(Agents/jarvis.tony/topics/rust-lang.jarvis.tony.md)"));
}

#[test]
fn rename_moves_self_references() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    // Write once so the note exists, then again so its self-links resolve and
    // are stored in the suffixed on-disk form.
    for content in ["seed", "I am [[rust]] and [me](topics/rust.md)"] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":content}),
        )
        .unwrap();
    }

    let body = structured(call(
        &tb,
        "rename_memory_note",
        json!({"agent":"jarvis","user":"tony",
               "path":"Agents/topics/rust.md","new_path":"Agents/topics/rust-lang.md"}),
    ));
    assert_eq!(body["notes_rewritten"], 0);

    // The moved note's own links point at the destination — the old name
    // neither dangles nor persists.
    assert_eq!(
        read_clean(&tb, "Agents/topics/rust-lang.md"),
        "I am [[rust-lang]] and [me](topics/rust-lang.md)"
    );
}

#[test]
fn rename_onto_existing_note_is_destination_exists() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for (path, content) in [
        ("Agents/notes/a.md", "alpha"),
        ("Agents/notes/b.md", "beta"),
    ] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":path,"content":content}),
        )
        .unwrap();
    }

    assert_code(
        call(
            &tb,
            "rename_memory_note",
            json!({"agent":"jarvis","user":"tony",
                   "path":"Agents/notes/a.md","new_path":"Agents/notes/b.md"}),
        ),
        "destination_exists",
    );
    // Neither note was modified.
    assert_eq!(read_clean(&tb, "Agents/notes/a.md"), "alpha");
    assert_eq!(read_clean(&tb, "Agents/notes/b.md"), "beta");
}

#[test]
fn rename_rejects_root_reserved_paths_on_both_ends() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/a.md","content":"x"}),
    )
    .unwrap();

    assert_code(
        call(
            &tb,
            "rename_memory_note",
            json!({"agent":"jarvis","user":"tony",
                   "path":"Agents/MEMORY.md","new_path":"Agents/topics/m.md"}),
        ),
        "path_not_permitted",
    );
    assert_code(
        call(
            &tb,
            "rename_memory_note",
            json!({"agent":"jarvis","user":"tony",
                   "path":"Agents/topics/a.md","new_path":"Agents/HEARTBEAT.md"}),
        ),
        "path_not_permitted",
    );
    assert_eq!(read_clean(&tb, "Agents/topics/a.md"), "x");
}

#[test]
fn rename_outside_agents_folder_denied_under_namespaced() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "shared");
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/a.md","content":"x"}),
    )
    .unwrap();

    // Source outside the agents folder.
    assert_code(
        call(
            &tb,
            "rename_memory_note",
            json!({"agent":"jarvis","user":"tony",
                   "path":"Actions/release.md","new_path":"Actions/new.md"}),
        ),
        "write_denied",
    );
    // Destination outside the agents folder.
    assert_code(
        call(
            &tb,
            "rename_memory_note",
            json!({"agent":"jarvis","user":"tony",
                   "path":"Agents/topics/a.md","new_path":"Actions/a.md"}),
        ),
        "write_denied",
    );
    assert!(tmp.path().join("Actions/release.md").exists());
    assert!(!tmp.path().join("Actions/new.md").exists());
    assert_eq!(read_clean(&tb, "Agents/topics/a.md"), "x");
}

#[test]
fn rename_missing_source_is_not_found() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "rename_memory_note",
            json!({"agent":"jarvis","user":"tony",
                   "path":"Agents/topics/ghost.md","new_path":"Agents/topics/g.md"}),
        ),
        "not_found",
    );
}

#[test]
fn rename_shared_to_scoped_is_refused_when_shared_referrers_exist() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Readwrite);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Actions/release.md","content":"shared target"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Actions/index.md","content":"see [[release]]"}),
    )
    .unwrap();

    // Rewriting the shared referrer would persist the caller's scope suffix in
    // the shared region — the whole rename is refused.
    assert_code(
        call(
            &tb,
            "rename_memory_note",
            json!({"agent":"jarvis","user":"tony",
                   "path":"Actions/release.md","new_path":"Agents/topics/release.md"}),
        ),
        "write_denied",
    );
    assert_eq!(read_clean(&tb, "Actions/release.md"), "shared target");
    assert_eq!(read_clean(&tb, "Actions/index.md"), "see [[release]]");
    assert!(
        !tmp.path()
            .join("Agents/jarvis.tony/topics/release.jarvis.tony.md")
            .exists()
    );
}

#[test]
fn rename_leak_guard_applies_to_moved_content() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Readwrite);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/helper.md","content":"h"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"see [[helper]]"}),
    )
    .unwrap();

    // Moving the note outside would persist a scoped link in the shared region.
    assert_code(
        call(
            &tb,
            "rename_memory_note",
            json!({"agent":"jarvis","user":"tony",
                   "path":"Agents/topics/rust.md","new_path":"Actions/rust.md"}),
        ),
        "write_denied",
    );
    assert_eq!(read_clean(&tb, "Agents/topics/rust.md"), "see [[helper]]");
    assert!(!tmp.path().join("Actions/rust.md").exists());
}

#[test]
fn rename_recall_hits_new_path_only_without_watcher() {
    let tmp = TempDir::new().unwrap();
    let tb = frozen_recall_toolbox(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"the zyzzyva fact"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/memo.md","content":"see [[rust]]"}),
    )
    .unwrap();

    let hit_paths = |body: &Value| -> Vec<String> {
        body["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["path"].as_str().unwrap().to_string())
            .collect()
    };

    // First query builds the index; the hit is at the old path.
    let before = structured(call(
        &tb,
        "recall_memory_notes",
        json!({"agent":"jarvis","user":"tony","query":"zyzzyva"}),
    ));
    assert_eq!(hit_paths(&before), vec!["Agents/topics/rust.md"]);

    call(
        &tb,
        "rename_memory_note",
        json!({"agent":"jarvis","user":"tony",
               "path":"Agents/topics/rust.md","new_path":"Agents/topics/rust-lang.md"}),
    )
    .unwrap();

    // The frozen index can only have learned of the rename through the
    // server's own synchronous notifications — no watcher, no reconcile.
    let after = structured(call(
        &tb,
        "recall_memory_notes",
        json!({"agent":"jarvis","user":"tony","query":"zyzzyva"}),
    ));
    assert_eq!(hit_paths(&after), vec!["Agents/topics/rust-lang.md"]);
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

#[test]
fn write_append_extends_existing_note_verbatim() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/log.md","content":"- old fact\n"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/log.md",
               "content":"- new fact\n","append":true}),
    ));
    // No separator inserted; the count is the note's total size after the write.
    assert_eq!(
        read_clean(&tb, "Agents/topics/log.md"),
        "- old fact\n- new fact\n"
    );
    assert_eq!(body["bytes_written"], "- old fact\n- new fact\n".len());
}

#[test]
fn write_append_to_missing_note_creates_it() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let body = structured(call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/log.md",
               "content":"- first\n","append":true}),
    ));
    assert_eq!(read_clean(&tb, "Agents/topics/log.md"), "- first\n");
    assert_eq!(body["bytes_written"], "- first\n".len());
}

#[test]
fn write_append_round_trips_links() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    // The link target must exist for [[rust]] to resolve and expand.
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/rust.md","content":"seed"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/log.md","content":"intro\n"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/notes/log.md",
               "content":"see [[rust]]\n","append":true}),
    )
    .unwrap();
    // The appended fragment persists in the suffixed form and reads back clean.
    let raw = std::fs::read_to_string(
        tmp.path()
            .join("Agents/jarvis.tony/notes/log.jarvis.tony.md"),
    )
    .unwrap();
    assert!(raw.contains("[[rust.jarvis.tony]]"), "raw: {raw}");
    assert_eq!(
        read_clean(&tb, "Agents/notes/log.md"),
        "intro\nsee [[rust]]\n"
    );
}

#[test]
fn write_append_root_core_file_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/MEMORY.md",
                   "content":"x","append":true}),
        ),
        "path_not_permitted",
    );
    assert!(
        !tmp.path()
            .join("Agents/jarvis.tony/MEMORY.jarvis.tony.md")
            .exists()
    );
}

#[test]
fn write_append_outside_under_namespaced_is_denied_and_unchanged() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "original");
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Actions/release.md",
                   "content":"new","append":true}),
        ),
        "write_denied",
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("Actions/release.md")).unwrap(),
        "original"
    );
}

#[test]
fn write_append_hidden_target_is_path_not_permitted_and_creates_nothing() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/.hidden.md",
                   "content":"x","append":true}),
        ),
        "path_not_permitted",
    );
    assert!(
        !tmp.path()
            .join("Agents/jarvis.tony/topics/.hidden.jarvis.tony.md")
            .exists()
    );
}

#[test]
fn write_append_concurrent_appends_are_serialised() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    std::thread::scope(|s| {
        for n in 0..8 {
            let tb = &tb;
            s.spawn(move || {
                call(
                    tb,
                    "write_memory_note",
                    json!({"agent":"jarvis","user":"tony","path":"Agents/topics/log.md",
                           "content":format!("entry-{n}\n"),"append":true}),
                )
                .unwrap();
            });
        }
    });
    let contents = read_clean(&tb, "Agents/topics/log.md");
    // Every fragment landed exactly once; the per-target lock prevents lost updates.
    for n in 0..8 {
        assert_eq!(
            contents.matches(&format!("entry-{n}\n")).count(),
            1,
            "entry-{n} in: {contents}"
        );
    }
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

// --- read_note_properties / update_note_properties ---

/// Seed a note for the property tests and return its physical path on disk.
fn seed_note(tmp: &TempDir, tb: &Toolbox, content: &str) -> std::path::PathBuf {
    call(
        tb,
        "write_memory_note",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","content":content}),
    )
    .unwrap();
    tmp.path()
        .join("Agents/jarvis.tony/topics/n.jarvis.tony.md")
}

#[test]
fn properties_read_returns_frontmatter_as_json() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    seed_note(
        &tmp,
        &tb,
        "---\ntags: [rust, async]\nstatus: draft\n---\nbody\n",
    );
    let body = structured(call(
        &tb,
        "read_note_properties",
        json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md"}),
    ));
    assert_eq!(
        body["properties"],
        json!({ "tags": ["rust", "async"], "status": "draft" })
    );
}

#[test]
fn properties_read_empty_for_absent_or_malformed_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for content in ["no fence here\n", "---\n: : not valid : :\n---\nbody\n"] {
        seed_note(&tmp, &tb, content);
        let body = structured(call(
            &tb,
            "read_note_properties",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md"}),
        ));
        assert_eq!(body["properties"], json!({}), "for {content:?}");
    }
}

#[test]
fn properties_read_gating_matches_read_memory_note() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "read_note_properties",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/nope.md"}),
        ),
        "not_found",
    );
    assert_code(
        call(
            &tb,
            "read_note_properties",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/.secret.md"}),
        ),
        "path_not_permitted",
    );
    let scoped = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Scoped);
    write_outside(&tmp, "Actions/release.md", "shared");
    assert_code(
        call(
            &scoped,
            "read_note_properties",
            json!({"agent":"jarvis","user":"tony","path":"Actions/release.md"}),
        ),
        "path_not_permitted",
    );
}

#[test]
fn properties_read_root_core_file_is_readable() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "evolve_core_persona",
        json!({"agent":"jarvis","user":"tony","which":"persona","content":"soul"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_note_properties",
        json!({"agent":"jarvis","user":"tony","path":"Agents/PERSONA.md"}),
    ));
    assert_eq!(body["properties"], json!({}));
}

#[test]
fn properties_update_merges_and_returns_full_set_with_body_untouched() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let physical = seed_note(
        &tmp,
        &tb,
        "---\nstatus: draft\npriority: 2\n---\nThe body.\n",
    );
    let body = structured(call(
        &tb,
        "update_note_properties",
        json!({
            "agent":"jarvis","user":"tony","path":"Agents/topics/n.md",
            "properties": { "status": "done", "reviewed": true, "priority": null },
        }),
    ));
    assert_eq!(
        body["properties"],
        json!({ "status": "done", "reviewed": true })
    );
    // Normalized block (sorted keys), body byte-identical.
    assert_eq!(
        std::fs::read_to_string(physical).unwrap(),
        "---\nreviewed: true\nstatus: done\n---\nThe body.\n"
    );
}

#[test]
fn properties_update_creates_block_when_absent() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let physical = seed_note(&tmp, &tb, "Just the body.\n");
    call(
        &tb,
        "update_note_properties",
        json!({
            "agent":"jarvis","user":"tony","path":"Agents/topics/n.md",
            "properties": { "status": "draft" },
        }),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(physical).unwrap(),
        "---\nstatus: draft\n---\nJust the body.\n"
    );
}

#[test]
fn properties_update_removes_emptied_block() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let physical = seed_note(&tmp, &tb, "---\nstatus: draft\n---\nThe body.\n");
    let body = structured(call(
        &tb,
        "update_note_properties",
        json!({
            "agent":"jarvis","user":"tony","path":"Agents/topics/n.md",
            "properties": { "status": null },
        }),
    ));
    assert_eq!(body["properties"], json!({}));
    assert_eq!(std::fs::read_to_string(physical).unwrap(), "The body.\n");
}

#[test]
fn properties_update_malformed_fence_is_invalid_argument_and_unchanged() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    let content = "---\n: : not valid : :\n---\nbody\n";
    let physical = seed_note(&tmp, &tb, content);
    assert_code(
        call(
            &tb,
            "update_note_properties",
            json!({
                "agent":"jarvis","user":"tony","path":"Agents/topics/n.md",
                "properties": { "status": "done" },
            }),
        ),
        "invalid_argument",
    );
    assert_eq!(std::fs::read_to_string(physical).unwrap(), content);
}

#[test]
fn properties_update_root_core_file_is_reserved_naming_wrapper() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for (f, wrapper) in [
        ("MEMORY.md", "evolve_core_persona"),
        ("HEARTBEAT.md", "update_task_heartbeat"),
    ] {
        let res = call(
            &tb,
            "update_note_properties",
            json!({
                "agent":"jarvis","user":"tony","path":format!("Agents/{f}"),
                "properties": { "status": "done" },
            }),
        );
        match res {
            Err(e) => {
                assert_eq!(e.code().as_str(), "path_not_permitted", "for {f}");
                assert!(
                    e.to_string().contains(wrapper),
                    "message should name the wrapper for {f}: {e}"
                );
            }
            Ok(_) => panic!("expected rejection updating root {f}"),
        }
    }
}

#[test]
fn properties_update_outside_under_namespaced_is_denied_and_unchanged() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    write_outside(&tmp, "Actions/release.md", "---\nstatus: draft\n---\nx\n");
    assert_code(
        call(
            &tb,
            "update_note_properties",
            json!({
                "agent":"jarvis","user":"tony","path":"Actions/release.md",
                "properties": { "status": "done" },
            }),
        ),
        "write_denied",
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("Actions/release.md")).unwrap(),
        "---\nstatus: draft\n---\nx\n"
    );
}

#[test]
fn properties_update_hidden_is_path_not_permitted() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "update_note_properties",
            json!({
                "agent":"jarvis","user":"tony","path":"Agents/topics/.hidden.md",
                "properties": { "status": "done" },
            }),
        ),
        "path_not_permitted",
    );
}

#[test]
fn properties_update_missing_is_not_found() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "update_note_properties",
            json!({
                "agent":"jarvis","user":"tony","path":"Agents/topics/nope.md",
                "properties": { "status": "done" },
            }),
        ),
        "not_found",
    );
    // The merge never creates the file.
    assert!(
        !tmp.path()
            .join("Agents/jarvis.tony/topics/nope.jarvis.tony.md")
            .exists()
    );
}

#[cfg(feature = "recall-tantivy")]
#[test]
fn properties_update_is_immediately_recallable_via_filters() {
    let tmp = TempDir::new().unwrap();
    let tb = frozen_toolbox(&tmp, RecallBackendKind::Tantivy);
    call(
        &tb,
        "write_memory_note",
        json!({
            "agent":"jarvis","user":"tony","path":"Agents/topics/task.md",
            "content":"---\nstatus: draft\n---\nShip the feature.\n",
        }),
    )
    .unwrap();
    let filters = json!([{ "key": "status", "op": "eq", "value": "done" }]);
    // First query builds the index; the draft note does not match.
    let body = structured(call(
        &tb,
        "recall_memory_notes",
        json!({"agent":"jarvis","user":"tony","filters":filters}),
    ));
    assert_eq!(body["hits"], json!([]));
    // With the watcher off and the index frozen-fresh, only the synchronous
    // recall_on_write can make the updated value visible.
    call(
        &tb,
        "update_note_properties",
        json!({
            "agent":"jarvis","user":"tony","path":"Agents/topics/task.md",
            "properties": { "status": "done" },
        }),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "recall_memory_notes",
        json!({"agent":"jarvis","user":"tony","filters":filters}),
    ));
    let paths: Vec<&str> = body["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["Agents/topics/task.md"]);
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
    // Ordinary content hits carry the note's mtime as RFC 3339 UTC.
    let modified_at = top["modified_at"].as_str().expect("modified_at");
    assert!(chrono::DateTime::parse_from_rfc3339(modified_at).is_ok());
}

/// Set a note's mtime to the instant named by an RFC 3339 string.
fn set_mtime(tmp: &TempDir, rel: &str, rfc3339: &str) {
    let t: std::time::SystemTime = chrono::DateTime::parse_from_rfc3339(rfc3339)
        .unwrap()
        .into();
    std::fs::OpenOptions::new()
        .write(true)
        .open(tmp.path().join(rel))
        .unwrap()
        .set_modified(t)
        .unwrap();
}

#[test]
fn recall_time_only_returns_recent_notes_in_recency_order() {
    let tmp = TempDir::new().unwrap();
    let tb = recall_toolbox(&tmp);
    for (name, body) in [("old", "alpha"), ("new", "beta")] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":format!("Agents/topics/{name}.md"),"content":body}),
        )
        .unwrap();
    }
    set_mtime(
        &tmp,
        "Agents/jarvis.tony/topics/old.jarvis.tony.md",
        "2026-06-01T00:00:00Z",
    );
    set_mtime(
        &tmp,
        "Agents/jarvis.tony/topics/new.jarvis.tony.md",
        "2026-06-02T00:00:00Z",
    );

    // A bound equal to the older mtime is inclusive (half-open interval).
    let out = structured(call(
        &tb,
        "recall_memory_notes",
        json!({"agent":"jarvis","user":"tony","modified_after":"2026-06-01T00:00:00Z"}),
    ));
    let hits = out["hits"].as_array().unwrap();
    let paths: Vec<&str> = hits.iter().map(|h| h["path"].as_str().unwrap()).collect();
    assert_eq!(paths, vec!["Agents/topics/new.md", "Agents/topics/old.md"]);
    for hit in hits {
        assert_eq!(hit["score"], 1.0);
        assert!(hit["snippets"].as_array().unwrap().is_empty());
    }
    assert_eq!(hits[0]["modified_at"], "2026-06-02T00:00:00Z");

    // A `modified_before` equal to the newer mtime excludes it.
    let out = structured(call(
        &tb,
        "recall_memory_notes",
        json!({"agent":"jarvis","user":"tony","modified_before":"2026-06-02T00:00:00Z"}),
    ));
    let paths: Vec<&str> = out["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["Agents/topics/old.md"]);
}

#[test]
fn recall_date_only_bound_respects_configured_timezone() {
    // 20:00 UTC on June 9 is already June 10 in Asia/Taipei (UTC+8).
    let write_note = |tb: &Toolbox| {
        call(
            tb,
            "write_memory_note",
            json!({"agent":"jarvis","user":"tony","path":"Agents/topics/n.md","content":"x"}),
        )
        .unwrap();
    };
    let query = json!({"agent":"jarvis","user":"tony","modified_after":"2026-06-10"});

    let tmp = TempDir::new().unwrap();
    let tb = recall_toolbox_tz(&tmp, Tz::Asia__Taipei);
    write_note(&tb);
    set_mtime(
        &tmp,
        "Agents/jarvis.tony/topics/n.jarvis.tony.md",
        "2026-06-09T20:00:00Z",
    );
    let out = structured(call(&tb, "recall_memory_notes", query.clone()));
    assert_eq!(out["hits"].as_array().unwrap().len(), 1);

    let tmp = TempDir::new().unwrap();
    let tb = recall_toolbox_tz(&tmp, Tz::UTC);
    write_note(&tb);
    set_mtime(
        &tmp,
        "Agents/jarvis.tony/topics/n.jarvis.tony.md",
        "2026-06-09T20:00:00Z",
    );
    let out = structured(call(&tb, "recall_memory_notes", query));
    assert!(out["hits"].as_array().unwrap().is_empty());
}

#[test]
fn recall_invalid_time_bound_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let tb = recall_toolbox(&tmp);
    assert_code(
        call(
            &tb,
            "recall_memory_notes",
            json!({"agent":"jarvis","user":"tony","modified_after":"last tuesday"}),
        ),
        "invalid_argument",
    );
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
fn recall_requires_a_content_or_time_predicate() {
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
