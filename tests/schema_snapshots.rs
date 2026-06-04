//! Snapshot tests for the tool input schemas exposed via `tools/list`, across
//! several representative VFS templates (task 8.11).
//!
//! These lock the JSON shape of the template-derived scope fields merged with
//! each tool's own fields. Run `cargo insta review` to accept intentional changes.

use agentmem::path::PathResolver;
use agentmem::policy::Policy;
use agentmem::storage::Storage;
use agentmem::template::Template;
use agentmem::tools::Toolbox;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use chrono_tz::Tz;
use serde_json::{Value, json};

/// The `tools/list` schemas for a given template, as a name → inputSchema map.
fn schemas_for(template: &str) -> Value {
    let tmp = TempDir::new().unwrap();
    let resolver = PathResolver::new(
        tmp.path().canonicalize().unwrap(),
        Utf8PathBuf::from("Agents"),
        Template::parse(template).unwrap(),
    );
    let storage = Storage::new(resolver, true, false);
    let toolbox = Toolbox::new(storage, Policy::Namespaced, Tz::UTC);

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
fn schema_empty_template() {
    insta::assert_json_snapshot!("schema_empty_template", schemas_for(""));
}

#[test]
fn schema_agent_template() {
    insta::assert_json_snapshot!("schema_agent_template", schemas_for("<agent>"));
}

#[test]
fn schema_agent_user_template() {
    insta::assert_json_snapshot!("schema_agent_user_template", schemas_for("<agent>.<user>"));
}

#[test]
fn schema_team_agent_env_user_template() {
    insta::assert_json_snapshot!(
        "schema_team_agent_env_user_template",
        schemas_for("<team>.<agent>.<env>.<user>")
    );
}
