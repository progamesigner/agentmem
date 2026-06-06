//! The agent-facing tool surface: schema generation, scope extraction, and the
//! nine tool handlers.
//!
//! Each tool's input schema is assembled at startup by merging the
//! scheme-derived scope fields (see [`crate::scheme::Scheme::to_json_schema`])
//! with the tool-specific fields generated from a `schemars`-derived struct. At
//! call time the scope keys are extracted and validated, the virtual path is
//! resolved under the caller's own scope, the policy gate runs before any IO, and
//! visibility filters reject hidden/ignored targets.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use chrono_tz::Tz;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::error::AgentmemError;
use crate::path::VirtualPath;
use crate::policy::{Policy, PolicyError};
use crate::storage::{Cursor, Storage};
use crate::scheme::Scheme;

/// The default page size for `list_memory_notes`.
const DEFAULT_LIMIT: u64 = 200;
/// The maximum permitted page size.
const MAX_LIMIT: u64 = 1000;

/// The set of tool names this server exposes, in advertised order.
pub const TOOL_NAMES: &[&str] = &[
    "list_memory_notes",
    "read_memory_note",
    "write_memory_note",
    "edit_memory_note",
    "delete_memory_note",
    "load_session_context",
    "evolve_core_persona",
    "update_task_heartbeat",
    "append_diary_entry",
];

// --- schemars structs (tool-specific fields only; scope fields are merged in) ---

#[derive(JsonSchema, Deserialize)]
#[allow(dead_code)]
struct ListFields {
    /// Optional virtual-path prefix, relative to the agents folder, to filter by.
    #[serde(default)]
    path_prefix: Option<String>,
    /// Maximum number of entries to return (default 200, maximum 1000).
    #[serde(default)]
    limit: Option<u64>,
    /// Opaque pagination cursor returned by a previous call.
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct PathFields {
    /// The virtual path of the note, relative to the vault root.
    path: String,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct WriteFields {
    /// The virtual path of the note, relative to the vault root.
    path: String,
    /// The full new contents of the note.
    content: String,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct EditFields {
    /// The virtual path of the note, relative to the vault root.
    path: String,
    /// The exact string to replace. It must occur exactly once in the file.
    search_string: String,
    /// The replacement string.
    replace_string: String,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct EmptyFields {}

/// Which foundational session file `evolve_core_persona` targets.
#[derive(JsonSchema, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
enum Which {
    Persona,
    Prompt,
    Rules,
    User,
    Tools,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct EvolveFields {
    /// Which foundational file to replace.
    which: Which,
    /// The full new contents of the selected file.
    content: String,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct ContentOnlyFields {
    /// The content to write.
    content: String,
}

/// The agent-facing toolbox: holds the storage layer, the active policy, and the
/// configured timezone, and answers `tools/list` and `tools/call`.
pub struct Toolbox {
    storage: Storage,
    policy: Policy,
    timezone: Tz,
    tools: Vec<Tool>,
    /// Absolute path to the global session-context template file (may not exist).
    session_context_template_file: PathBuf,
}

impl Toolbox {
    /// Build the toolbox and precompute every tool's input schema for the active
    /// scheme.
    pub fn new(
        storage: Storage,
        policy: Policy,
        timezone: Tz,
        session_context_template_file: PathBuf,
    ) -> Toolbox {
        let scheme = storage.resolver().scheme().clone();
        let tools = build_tools(&scheme);
        Toolbox {
            storage,
            policy,
            timezone,
            tools,
            session_context_template_file,
        }
    }

    /// The advertised tool list for `tools/list`.
    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools.clone()
    }

    /// Dispatch a `tools/call`. Returns `None` when the tool name is unknown so
    /// the caller can map it to a protocol-level "method not found"; `Some(Err)`
    /// carries a domain error to surface as a structured tool result.
    pub fn call(
        &self,
        name: &str,
        args: &JsonObject,
    ) -> Option<Result<CallToolResult, AgentmemError>> {
        let result = match name {
            "list_memory_notes" => self.list_memory_notes(args),
            "read_memory_note" => self.read_memory_note(args),
            "write_memory_note" => self.write_memory_note(args),
            "edit_memory_note" => self.edit_memory_note(args),
            "delete_memory_note" => self.delete_memory_note(args),
            "load_session_context" => self.load_session_context(args),
            "evolve_core_persona" => self.evolve_core_persona(args),
            "update_task_heartbeat" => self.update_task_heartbeat(args),
            "append_diary_entry" => self.append_diary_entry(args),
            _ => return None,
        };
        Some(result)
    }

    // --- scope + argument helpers ---

    fn scheme(&self) -> &Scheme {
        self.storage.resolver().scheme()
    }

    /// Extract and validate the scope keys from the arguments, rejecting any
    /// argument key that is neither a scope placeholder nor a known tool field.
    /// Returns the validated scope map (placeholder ident → value).
    fn scope_map(
        &self,
        args: &JsonObject,
        tool_fields: &[&str],
    ) -> Result<BTreeMap<String, String>, AgentmemError> {
        let placeholders = self.scheme().placeholders();

        let mut scope: BTreeMap<String, String> = BTreeMap::new();
        for ph in &placeholders {
            match args.get(*ph) {
                Some(Value::String(s)) if !s.is_empty() => {
                    scope.insert((*ph).to_string(), s.clone());
                }
                Some(Value::String(_)) => {
                    return Err(AgentmemError::InvalidArgument {
                        message: format!("scope key '{ph}' must not be empty"),
                    });
                }
                Some(_) => {
                    return Err(AgentmemError::InvalidArgument {
                        message: format!("scope key '{ph}' must be a string"),
                    });
                }
                None => {
                    return Err(AgentmemError::MissingScope {
                        key: (*ph).to_string(),
                    });
                }
            }
        }

        for key in args.keys() {
            let is_scope = placeholders.contains(&key.as_str());
            let is_field = tool_fields.contains(&key.as_str());
            if !is_scope && !is_field {
                return Err(AgentmemError::InvalidArgument {
                    message: format!("unexpected parameter '{key}'"),
                });
            }
        }

        Ok(scope)
    }

    /// Extract, validate, and render the scope suffix from the arguments.
    fn resolve_scope(
        &self,
        args: &JsonObject,
        tool_fields: &[&str],
    ) -> Result<String, AgentmemError> {
        let scope = self.scope_map(args, tool_fields)?;
        self.scheme()
            .render(&scope)
            .map_err(|e| AgentmemError::InvalidArgument {
                message: e.to_string(),
            })
    }

    /// The scheme's placeholder idents, in order — the scope keys every surface
    /// (tool, resource, prompt) requires. Used by the MCP server to derive the
    /// resource URI parameters and the prompt arguments.
    pub fn scheme_placeholders(&self) -> Vec<String> {
        self.scheme()
            .placeholders()
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    }

    /// Render the session-context for a pre-built scope map (used by the resource
    /// and prompt surfaces). Validates that `scope` contains exactly the scheme's
    /// placeholder keys before rendering.
    pub fn render_session_context(
        &self,
        scope: &BTreeMap<String, String>,
    ) -> Result<crate::session_context::SessionContext, AgentmemError> {
        for ph in &self.scheme().placeholders() {
            match scope.get(*ph) {
                Some(v) if !v.is_empty() => {}
                Some(_) => {
                    return Err(AgentmemError::InvalidArgument {
                        message: format!("scope key '{ph}' must not be empty"),
                    });
                }
                None => {
                    return Err(AgentmemError::MissingScope {
                        key: (*ph).to_string(),
                    });
                }
            }
        }
        let placeholders = self.scheme().placeholders();
        for key in scope.keys() {
            if !placeholders.contains(&key.as_str()) {
                return Err(AgentmemError::InvalidArgument {
                    message: format!("unexpected scope key '{key}'"),
                });
            }
        }
        crate::session_context::render_session_context(
            &self.storage,
            &self.session_context_template_file,
            &self.tools,
            scope,
        )
    }

    /// The clean virtual path of a conventional file relative to the agents
    /// folder (e.g. `Agents/PERSONA.md`, or `PERSONA.md` when the agents folder is
    /// the vault root).
    fn agents_vpath(&self, relative: &str) -> Result<VirtualPath, AgentmemError> {
        let agents = self.storage.resolver().agents_dir();
        let full = if agents.as_str().is_empty() {
            relative.to_string()
        } else {
            format!("{agents}/{relative}")
        };
        VirtualPath::new(&full)
    }

    /// Map a [`PolicyError`] to the appropriate boundary error for `vpath`.
    fn policy_err(err: PolicyError, vpath: &VirtualPath) -> AgentmemError {
        match err {
            PolicyError::NotPermitted => AgentmemError::PathNotPermitted {
                virtual_path: vpath.as_str().to_string(),
            },
            PolicyError::WriteDenied => AgentmemError::WriteDenied {
                virtual_path: vpath.as_str().to_string(),
            },
        }
    }

    // --- handlers ---

    fn list_memory_notes(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["path_prefix", "limit", "cursor"])?;

        let path_prefix = opt_str(args, "path_prefix")?;
        let limit = match opt_u64(args, "limit")? {
            Some(n) if n > MAX_LIMIT => {
                return Err(AgentmemError::InvalidArgument {
                    message: format!("limit must not exceed {MAX_LIMIT}"),
                });
            }
            Some(n) => n.max(1),
            None => DEFAULT_LIMIT,
        };
        let offset = match opt_str(args, "cursor")? {
            Some(c) => Cursor::decode(&c)?,
            None => 0,
        };

        let regions = self.policy.list_visible_regions(self.scheme().is_empty());
        let mut items: Vec<String> = self
            .storage
            .list_visible(&scope, &regions)?
            .into_iter()
            .map(|p| p.as_str().to_string())
            .collect();

        if let Some(prefix) = &path_prefix {
            let agents = self.storage.resolver().agents_dir();
            let effective = if agents.as_str().is_empty() {
                prefix.clone()
            } else {
                format!("{agents}/{prefix}")
            };
            let with_sep = format!("{effective}/");
            items.retain(|p| p == &effective || p.starts_with(&with_sep));
        }

        let total = items.len() as u64;
        let start = offset.min(total) as usize;
        let end = (offset + limit).min(total) as usize;
        let page: Vec<String> = items[start..end].to_vec();
        let next_cursor = if (end as u64) < total {
            Some(Cursor::encode(end as u64))
        } else {
            None
        };

        Ok(ok_json(json!({
            "items": page,
            "next_cursor": next_cursor,
        })))
    }

    fn read_memory_note(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["path"])?;
        let vpath = VirtualPath::new(&require_str(args, "path")?)?;
        let resolver = self.storage.resolver();
        let region = resolver.detect_region(&vpath);
        self.policy
            .gate_read(region)
            .map_err(|e| Self::policy_err(e, &vpath))?;
        let physical = resolver.resolve(&scope, &vpath)?;
        if !self.storage.is_visible(&physical) {
            return Err(AgentmemError::PathNotPermitted {
                virtual_path: vpath.as_str().to_string(),
            });
        }
        let content = self.storage.read(&physical)?;
        let mut result = CallToolResult::success(vec![Content::text(content.clone())]);
        result.structured_content = Some(json!({ "content": content }));
        Ok(result)
    }

    fn write_memory_note(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["path", "content"])?;
        let vpath = VirtualPath::new(&require_str(args, "path")?)?;
        let content = require_str(args, "content")?;
        self.gated_write(&scope, &vpath, |physical, storage| {
            storage.write_atomic(physical, &content)
        })
    }

    fn edit_memory_note(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["path", "search_string", "replace_string"])?;
        let vpath = VirtualPath::new(&require_str(args, "path")?)?;
        let search = require_str(args, "search_string")?;
        let replace = require_str(args, "replace_string")?;
        let resolver = self.storage.resolver();
        let region = resolver.detect_region(&vpath);
        self.policy
            .gate_write(region)
            .map_err(|e| Self::policy_err(e, &vpath))?;
        let physical = resolver.resolve(&scope, &vpath)?;
        if !self.storage.is_visible(&physical) {
            return Err(AgentmemError::PathNotPermitted {
                virtual_path: vpath.as_str().to_string(),
            });
        }
        let replaced = self
            .storage
            .edit_search_replace(&physical, &search, &replace)?;
        Ok(ok_json(json!({ "chars_replaced": replaced })))
    }

    fn delete_memory_note(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["path"])?;
        let vpath = VirtualPath::new(&require_str(args, "path")?)?;
        let resolver = self.storage.resolver();
        let region = resolver.detect_region(&vpath);
        self.policy
            .gate_write(region)
            .map_err(|e| Self::policy_err(e, &vpath))?;
        let physical = resolver.resolve(&scope, &vpath)?;
        if !self.storage.is_visible(&physical) {
            return Err(AgentmemError::PathNotPermitted {
                virtual_path: vpath.as_str().to_string(),
            });
        }
        self.storage.delete(&physical)?;
        Ok(ok_json(json!({ "deleted": true })))
    }

    fn load_session_context(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        // Accept only scope parameters (no `path`/`which`).
        let scope = self.scope_map(args, &[])?;
        let sc = crate::session_context::render_session_context(
            &self.storage,
            &self.session_context_template_file,
            &self.tools,
            &scope,
        )?;
        Ok(ok_json(json!({ "rendered": sc.rendered, "missing": sc.missing })))
    }

    fn evolve_core_persona(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["which", "content"])?;
        let which = require_str(args, "which")?;
        let filename = match which.as_str() {
            "persona" => "PERSONA.md",
            "prompt" => "PROMPT.md",
            "rules" => "RULES.md",
            "user" => "USER.md",
            "tools" => "TOOLS.md",
            other => {
                return Err(AgentmemError::InvalidArgument {
                    message: format!(
                        "which must be one of persona|prompt|rules|user|tools, got '{other}'"
                    ),
                });
            }
        };
        let content = require_str(args, "content")?;
        let vpath = self.agents_vpath(filename)?;
        self.gated_write(&scope, &vpath, |physical, storage| {
            storage.write_atomic(physical, &content)
        })
    }

    fn update_task_heartbeat(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["content"])?;
        let content = require_str(args, "content")?;
        let vpath = self.agents_vpath("HEARTBEAT-STATE.md")?;
        self.gated_write(&scope, &vpath, |physical, storage| {
            storage.write_atomic(physical, &content)
        })
    }

    fn append_diary_entry(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["content"])?;
        let content = require_str(args, "content")?;
        if content.is_empty() {
            return Err(AgentmemError::InvalidArgument {
                message: "content must not be empty".to_string(),
            });
        }
        let now = Utc::now().with_timezone(&self.timezone);
        let date = now.format("%Y-%m-%d").to_string();
        let time = now.format("%H:%M:%S").to_string();
        let vpath = self.agents_vpath(&format!("diary/{date}.md"))?;

        let resolver = self.storage.resolver();
        let region = resolver.detect_region(&vpath);
        self.policy
            .gate_write(region)
            .map_err(|e| Self::policy_err(e, &vpath))?;
        let physical = resolver.resolve(&scope, &vpath)?;
        if !self.storage.is_visible(&physical) {
            return Err(AgentmemError::PathNotPermitted {
                virtual_path: vpath.as_str().to_string(),
            });
        }
        let written = self
            .storage
            .read_modify_write(&physical, |current| match current {
                Some(existing) => format!("{existing}\n## {time}\n{content}\n"),
                None => format!("## {time}\n{content}\n"),
            })?;
        Ok(ok_json(json!({ "bytes_written": written })))
    }

    /// Shared write path: gate by policy, enforce visibility, then run `op`.
    fn gated_write(
        &self,
        scope: &str,
        vpath: &VirtualPath,
        op: impl FnOnce(&crate::path::PhysicalPath, &Storage) -> Result<usize, AgentmemError>,
    ) -> Result<CallToolResult, AgentmemError> {
        let resolver = self.storage.resolver();
        let region = resolver.detect_region(vpath);
        self.policy
            .gate_write(region)
            .map_err(|e| Self::policy_err(e, vpath))?;
        let physical = resolver.resolve(scope, vpath)?;
        if !self.storage.is_visible(&physical) {
            return Err(AgentmemError::PathNotPermitted {
                virtual_path: vpath.as_str().to_string(),
            });
        }
        let written = op(&physical, &self.storage)?;
        Ok(ok_json(json!({ "bytes_written": written })))
    }
}

// --- free helpers ---

/// Build a successful tool result whose text is the JSON form and whose
/// structured content is `value`.
fn ok_json(value: Value) -> CallToolResult {
    let text = serde_json::to_string(&value).unwrap_or_default();
    let mut result = CallToolResult::success(vec![Content::text(text)]);
    result.structured_content = Some(value);
    result
}

/// Require a string argument, erroring with `invalid_argument` when absent or of
/// the wrong type.
fn require_str(args: &JsonObject, key: &str) -> Result<String, AgentmemError> {
    match args.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(AgentmemError::InvalidArgument {
            message: format!("argument '{key}' must be a string"),
        }),
        None => Err(AgentmemError::InvalidArgument {
            message: format!("missing required argument '{key}'"),
        }),
    }
}

fn opt_str(args: &JsonObject, key: &str) -> Result<Option<String>, AgentmemError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(AgentmemError::InvalidArgument {
            message: format!("argument '{key}' must be a string"),
        }),
    }
}

fn opt_u64(args: &JsonObject, key: &str) -> Result<Option<u64>, AgentmemError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => {
            n.as_u64()
                .map(Some)
                .ok_or_else(|| AgentmemError::InvalidArgument {
                    message: format!("argument '{key}' must be a non-negative integer"),
                })
        }
        Some(_) => Err(AgentmemError::InvalidArgument {
            message: format!("argument '{key}' must be an integer"),
        }),
    }
}

/// Generate the tool-specific field schema fragment via `schemars`, with all
/// subschemas inlined so enums appear directly rather than via `$ref`.
fn fields_schema<T: JsonSchema>() -> JsonObject {
    let mut settings = SchemaSettings::draft2020_12();
    settings.inline_subschemas = true;
    let generator = settings.into_generator();
    let schema = generator.into_root_schema_for::<T>();
    match serde_json::to_value(schema) {
        Ok(Value::Object(object)) => object,
        _ => Map::new(),
    }
}

/// Merge the scheme-derived scope fields (first) with a tool's own field schema
/// into a single object input schema with `additionalProperties: false`.
fn merge_schema(scheme: &Scheme, fields: JsonObject) -> JsonObject {
    let scope = scheme.to_json_schema();

    let mut properties = Map::new();
    if let Some(Value::Object(sp)) = scope.get("properties") {
        for (k, v) in sp {
            properties.insert(k.clone(), v.clone());
        }
    }
    if let Some(Value::Object(fp)) = fields.get("properties") {
        for (k, v) in fp {
            properties.insert(k.clone(), v.clone());
        }
    }

    let mut required = Vec::new();
    if let Some(Value::Array(sr)) = scope.get("required") {
        required.extend(sr.iter().cloned());
    }
    if let Some(Value::Array(fr)) = fields.get("required") {
        required.extend(fr.iter().cloned());
    }

    let mut out = Map::new();
    out.insert("type".to_string(), json!("object"));
    out.insert("properties".to_string(), Value::Object(properties));
    out.insert("required".to_string(), Value::Array(required));
    out.insert("additionalProperties".to_string(), json!(false));
    out
}

fn tool(name: &'static str, description: &'static str, schema: JsonObject) -> Tool {
    Tool {
        name: Cow::Borrowed(name),
        title: None,
        description: Some(Cow::Borrowed(description)),
        input_schema: Arc::new(schema),
        output_schema: None,
        annotations: None,
        icons: None,
        meta: None,
    }
}

/// Assemble the full nine-tool list for a given scheme.
fn build_tools(scheme: &Scheme) -> Vec<Tool> {
    vec![
        tool(
            "list_memory_notes",
            "List the virtual paths of memory notes visible to the given scope, with pagination.",
            merge_schema(scheme, fields_schema::<ListFields>()),
        ),
        tool(
            "read_memory_note",
            "Read the UTF-8 contents of a single memory note by its virtual path.",
            merge_schema(scheme, fields_schema::<PathFields>()),
        ),
        tool(
            "write_memory_note",
            "Atomically write the full contents of a memory note at the given virtual path.",
            merge_schema(scheme, fields_schema::<WriteFields>()),
        ),
        tool(
            "edit_memory_note",
            "Replace the unique occurrence of a search string in a note and persist atomically.",
            merge_schema(scheme, fields_schema::<EditFields>()),
        ),
        tool(
            "delete_memory_note",
            "Delete a single memory note by its virtual path.",
            merge_schema(scheme, fields_schema::<PathFields>()),
        ),
        tool(
            "load_session_context",
            "Render the session-context for the active scope: the foundational files woven into the configured template with a memory-tools guide. Returns { rendered, missing }.",
            merge_schema(scheme, fields_schema::<EmptyFields>()),
        ),
        tool(
            "evolve_core_persona",
            "Atomically replace one foundational session file selected by the `which` parameter.",
            merge_schema(scheme, fields_schema::<EvolveFields>()),
        ),
        tool(
            "update_task_heartbeat",
            "Atomically replace the scope's HEARTBEAT-STATE.md.",
            merge_schema(scheme, fields_schema::<ContentOnlyFields>()),
        ),
        tool(
            "append_diary_entry",
            "Append a timestamped section to today's diary file for the active scope.",
            merge_schema(scheme, fields_schema::<ContentOnlyFields>()),
        ),
    ]
}
