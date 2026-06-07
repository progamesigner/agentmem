//! Snapshot tests for the tool input schemas exposed via `tools/list`, across
//! several representative VFS schemes (task 8.11).
//!
//! These lock the JSON shape of the scheme-derived scope fields merged with
//! each tool's own fields. Run `cargo insta review` to accept intentional changes.

use agentmem::path::PathResolver;
use agentmem::policy::Policy;
use agentmem::scheme::Scheme;
use agentmem::storage::Storage;
use agentmem::tools::Toolbox;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use chrono_tz::Tz;
use serde_json::{Value, json};

/// The `tools/list` schemas for a given scheme, as a name → inputSchema map.
fn schemas_for(scheme: &str) -> Value {
    let tmp = TempDir::new().unwrap();
    let resolver = PathResolver::new(
        tmp.path().canonicalize().unwrap(),
        Utf8PathBuf::from("Agents"),
        Scheme::parse(scheme).unwrap(),
    );
    let storage = Storage::new(resolver, true, false, &[]);
    let toolbox = Toolbox::new(
        storage,
        Policy::Namespaced,
        Tz::UTC,
        tmp.path().join("AGENT_SESSION_CONTEXT.md"),
    );

    let mut map = serde_json::Map::new();
    for tool in toolbox.list_tools() {
        map.insert(
            tool.name.to_string(),
            json!({
                "description": tool.description,
                "input_schema": &*tool.input_schema,
            }),
        );
    }
    Value::Object(map)
}

#[test]
fn schema_empty_scheme() {
    insta::assert_json_snapshot!("schema_empty_scheme", schemas_for(""));
}

#[test]
fn schema_agent_scheme() {
    insta::assert_json_snapshot!("schema_agent_scheme", schemas_for("<agent>"));
}

#[test]
fn schema_agent_user_scheme() {
    insta::assert_json_snapshot!("schema_agent_user_scheme", schemas_for("<agent>.<user>"));
}

#[test]
fn schema_team_agent_env_user_scheme() {
    insta::assert_json_snapshot!(
        "schema_team_agent_env_user_scheme",
        schemas_for("<team>.<agent>.<env>.<user>")
    );
}
