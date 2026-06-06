//! In-process tool-handler tests covering every scenario in
//! `specs/memory-tools/spec.md` (task 8.12).
//!
//! The tests drive [`Toolbox`] directly — the same code path the MCP `call_tool`
//! handler uses — so each scenario exercises scope extraction, path resolution,
//! policy gating, visibility filtering, and storage end to end.

use agentmem::AgentmemError;
use agentmem::path::PathResolver;
use agentmem::policy::Policy;
use agentmem::storage::Storage;
use agentmem::scheme::Scheme;
use agentmem::tools::Toolbox;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use chrono_tz::Tz;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn toolbox(tmp: &TempDir, agents: &str, scheme: &str, policy: Policy) -> Toolbox {
    let resolver = PathResolver::new(
        tmp.path().canonicalize().unwrap(),
        Utf8PathBuf::from(agents),
        Scheme::parse(scheme).unwrap(),
    );
    let storage = Storage::new(resolver, true, false);
    Toolbox::new(
        storage,
        policy,
        Tz::UTC,
        tmp.path().join("AGENT_SESSION_CONTEXT.md"),
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
        json!({"agent":"coder","user":"alice","path":"Agents/notes.md","content":"x"}),
    )
    .unwrap();
    write_outside(&tmp, "Actions/release.md", "shared");

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(items.contains(&"Agents/notes.md"));
    assert!(items.contains(&"Actions/release.md"));
}

#[test]
fn list_path_prefix_filters_inside_agents_folder() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/topics/rust.md","content":"x"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/other/go.md","content":"x"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice","path_prefix":"topics"}),
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
        json!({"agent":"coder","user":"alice","path":"Agents/notes.md","content":"a"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"coder","user":"bob","path":"Agents/notes.md","content":"b"}),
    )
    .unwrap();

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items, vec!["Agents/notes.md"]);
}

#[test]
fn list_scoped_policy_hides_outside() {
    let tmp = TempDir::new().unwrap();
    let tb = toolbox(&tmp, "Agents", "<agent>.<user>", Policy::Scoped);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/notes.md","content":"x"}),
    )
    .unwrap();
    write_outside(&tmp, "Actions/release.md", "shared");

    let body = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice"}),
    ));
    let items: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(items, vec!["Agents/notes.md"]);
}

#[test]
fn list_paginates_with_limit_and_cursor() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for i in 0..5 {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"coder","user":"alice","path":format!("Agents/n{i}.md"),"content":"x"}),
        )
        .unwrap();
    }
    let page1 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice","limit":2}),
    ));
    assert_eq!(page1["items"].as_array().unwrap().len(), 2);
    let cursor = page1["next_cursor"]
        .as_str()
        .expect("cursor on first page")
        .to_string();

    let page2 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice","limit":2,"cursor":cursor}),
    ));
    assert_eq!(page2["items"].as_array().unwrap().len(), 2);

    let cursor2 = page2["next_cursor"].as_str().unwrap().to_string();
    let page3 = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice","limit":2,"cursor":cursor2}),
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
            json!({"agent":"coder","user":"alice","limit":1001}),
        ),
        "invalid_argument",
    );
}

#[test]
fn list_ordering_is_stable() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for name in ["c", "a", "b"] {
        call(&tb, "write_memory_note", json!({"agent":"coder","user":"alice","path":format!("Agents/{name}.md"),"content":"x"})).unwrap();
    }
    let a = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice"}),
    ));
    let b = structured(call(
        &tb,
        "list_memory_notes",
        json!({"agent":"coder","user":"alice"}),
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
        json!({"agent":"coder","user":"alice","path":"Agents/PERSONA.md","content":"hi"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/PERSONA.md"}),
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
        json!({"agent":"coder","user":"alice","path":"Actions/release.md"}),
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
            json!({"agent":"coder","user":"alice","path":"Actions/release.md"}),
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
            json!({"agent":"coder","user":"alice","path":"Agents/nope.md"}),
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
        json!({"agent":"coder","user":"alice","path":"Agents/.secret.md"}),
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
        json!({"agent":"coder","user":"alice","path":"Agents/n.md","content":"hello"}),
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
            json!({"agent":"coder","user":"alice","path":"Actions/release.md","content":"new"}),
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
        json!({"agent":"coder","user":"alice","path":"Scratch/team-notes.md","content":"shared"}),
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
            json!({"agent":"coder","user":"alice","path":"Agents/n.md","content":"x"}),
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
            json!({"agent":"coder","user":"alice","path":"Agents/.hidden.md","content":"x"}),
        ),
        "path_not_permitted",
    );
    assert!(
        !tmp.path()
            .join("Agents/coder.alice/.hidden.coder.alice.md")
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
        json!({"agent":"coder","user":"alice","path":"Agents/n.md","content":"alpha beta gamma"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "edit_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/n.md","search_string":"beta","replace_string":"BETA"}),
    ));
    assert_eq!(body["chars_replaced"], 4);
    let read = structured(call(
        &tb,
        "read_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/n.md"}),
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
            json!({"agent":"coder","user":"alice","path":"Actions/release.md","search_string":"beta","replace_string":"x"}),
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
        json!({"agent":"coder","user":"alice","path":"Agents/n.md","content":"alpha"}),
    )
    .unwrap();
    assert_code(
        call(
            &tb,
            "edit_memory_note",
            json!({"agent":"coder","user":"alice","path":"Agents/n.md","search_string":"zeta","replace_string":"x"}),
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
        json!({"agent":"coder","user":"alice","path":"Agents/n.md","content":"dup dup"}),
    )
    .unwrap();
    assert_code(
        call(
            &tb,
            "edit_memory_note",
            json!({"agent":"coder","user":"alice","path":"Agents/n.md","search_string":"dup","replace_string":"x"}),
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
        json!({"agent":"coder","user":"alice","path":"Agents/n.md","content":"x"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "delete_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/n.md"}),
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
            json!({"agent":"coder","user":"alice","path":"Agents/n.md"}),
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
            json!({"agent":"coder","user":"alice","path":"Actions/release.md"}),
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
            json!({"agent":"coder","user":"alice","path":"Actions/release.md"}),
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
            json!({"agent":"coder","user":"alice","path":"Agents/nope.md"}),
        ),
        "not_found",
    );
}

#[test]
fn delete_other_scope_is_unreachable() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    // bob owns the file; alice's delete of the same logical name must miss it.
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"coder","user":"bob","path":"Agents/n.md","content":"bob"}),
    )
    .unwrap();
    assert_code(
        call(
            &tb,
            "delete_memory_note",
            json!({"agent":"coder","user":"alice","path":"Agents/n.md"}),
        ),
        "not_found",
    );
    assert!(tmp.path().join("Agents/coder.bob/n.coder.bob.md").exists());
}

// --- load_session_context ---

#[test]
fn load_session_context_all_present() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    for f in ["PERSONA.md", "PROMPT.md", "RULES.md", "USER.md", "TOOLS.md"] {
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"coder","user":"alice","path":format!("Agents/{f}"),"content":f}),
        )
        .unwrap();
    }
    let body = structured(call(
        &tb,
        "load_session_context",
        json!({"agent":"coder","user":"alice"}),
    ));
    let rendered = body["rendered"].as_str().unwrap();
    // Each foundational file's contents are woven into the rendered output.
    assert!(rendered.contains("PERSONA.md"));
    assert!(rendered.contains("TOOLS.md"));
    assert_eq!(body["missing"].as_array().unwrap().len(), 0);
}

#[test]
fn load_session_context_some_missing() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/PERSONA.md","content":"p"}),
    )
    .unwrap();
    call(
        &tb,
        "write_memory_note",
        json!({"agent":"coder","user":"alice","path":"Agents/RULES.md","content":"r"}),
    )
    .unwrap();
    let body = structured(call(
        &tb,
        "load_session_context",
        json!({"agent":"coder","user":"alice"}),
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
            && missing.contains(&"TOOLS.md")
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
            json!({"agent":"coder","user":"alice","path":"x"}),
        ),
        "invalid_argument",
    );
    assert_code(
        call(
            &tb,
            "load_session_context",
            json!({"agent":"coder","user":"alice","which":"persona"}),
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
        ("tools", "TOOLS.md"),
    ] {
        call(
            &tb,
            "evolve_core_persona",
            json!({"agent":"coder","user":"alice","which":which,"content":which}),
        )
        .unwrap();
        let physical = tmp.path().join(format!(
            "Agents/coder.alice/{}",
            file.replace(".md", ".coder.alice.md")
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
            json!({"agent":"coder","user":"alice","which":"bogus","content":"x"}),
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
            json!({"agent":"coder","user":"alice","which":"persona","content":"x","path":"y"}),
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
            json!({"agent":"coder","user":"alice","which":"persona","content":"x"}),
        ),
        "write_denied",
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
        json!({"agent":"coder","user":"alice","content":"working"}),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(
            tmp.path()
                .join("Agents/coder.alice/HEARTBEAT-STATE.coder.alice.md")
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
        json!({"agent":"coder","user":"alice","content":"first"}),
    )
    .unwrap();
    call(
        &tb,
        "append_diary_entry",
        json!({"agent":"coder","user":"alice","content":"second"}),
    )
    .unwrap();
    // Find the single diary file under the scope dir.
    let diary_dir = tmp.path().join("Agents/coder.alice/diary");
    let file = std::fs::read_dir(&diary_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let contents = std::fs::read_to_string(file).unwrap();
    assert!(contents.starts_with("## "));
    assert!(contents.contains("first"));
    assert!(contents.contains("second"));
    // Two sections.
    assert_eq!(contents.matches("\n## ").count() + 1, 2);
}

#[test]
fn diary_rejects_path_arg() {
    let tmp = TempDir::new().unwrap();
    let tb = default_tb(&tmp);
    assert_code(
        call(
            &tb,
            "append_diary_entry",
            json!({"agent":"coder","user":"alice","content":"x","path":"y"}),
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
            json!({"agent":"coder","user":"alice","content":""}),
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
        json!({"agent":"coder","path":"Agents/n.md"}),
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
            json!({"agent":"coder","user":"alice","path":"Agents/n.md"}),
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
    call(&tb, "write_memory_note", json!({"team":"platform","agent":"coder","env":"prod","user":"alice","path":"Agents/plan.md","content":"x"})).unwrap();
    assert!(
        tmp.path()
            .join("Agents/platform.coder.prod.alice/plan.platform.coder.prod.alice.md")
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
        json!({"path":"Agents/n.md","content":"x"}),
    )
    .unwrap();
    assert!(tmp.path().join("Agents/n.md").exists());
    // Supplying a scope field is rejected.
    assert_code(
        call(
            &tb,
            "write_memory_note",
            json!({"agent":"coder","path":"Agents/n.md","content":"x"}),
        ),
        "invalid_argument",
    );
}
