//! The agent-facing tool surface: schema generation, scope extraction, and the
//! tool handlers (the nine memory-note tools plus `recall_memory_notes` when the
//! recall backend is enabled).
//!
//! Each tool's input schema is assembled at startup by merging the
//! scheme-derived scope fields (see [`crate::scheme::Scheme::to_json_schema`])
//! with the tool-specific fields generated from a `schemars`-derived struct. At
//! call time the scope keys are extracted and validated, the virtual path is
//! resolved under the caller's own scope, the policy gate runs before any IO, and
//! visibility filters reject hidden/ignored targets.

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
use crate::path::{PhysicalPath, VirtualPath};
use crate::policy::{Policy, PolicyError, Region};
use crate::recall::{FilterOp, PropertyFilter, RecallEngine, RecallQuery};
use crate::scheme::Scheme;
use crate::storage::{Cursor, Storage};

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
    /// Optional glob pattern matched against each entry's clean, vault-root-relative
    /// virtual path (e.g. `Agents/diary/2026-*`, `**/release.md`). Supports `*`, `**`,
    /// `?`, and character classes. Composes with `path_prefix` via AND (both must
    /// match). Filters paths only; reads no note contents.
    #[serde(default)]
    glob: Option<String>,
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
    Memory,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct EvolveFields {
    /// Which foundational file to replace: one of persona, prompt, rules, user, memory.
    which: Which,
    /// The full new contents of the selected file. `user` content must be ≤ 100
    /// lines and `memory` content ≤ 200 lines.
    content: String,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct ContentOnlyFields {
    /// The content to write.
    content: String,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct DiaryFields {
    /// The diary entry body.
    content: String,
    /// Optional short title for the entry; when present the heading reads
    /// `## <HH:MM:SS> — <title>`, otherwise `## <HH:MM:SS>`.
    title: Option<String>,
}

/// The comparison a frontmatter property filter applies.
#[derive(JsonSchema, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
enum FilterOpField {
    Exists,
    Eq,
    Contains,
    Gt,
    Lt,
    Ge,
    Le,
}

/// One frontmatter property predicate (tantivy backend only).
#[derive(JsonSchema)]
#[allow(dead_code)]
struct PropertyFilterField {
    /// The frontmatter property key.
    key: String,
    /// The comparison: exists, eq, contains, gt, lt, ge, le.
    op: FilterOpField,
    /// The value to compare against (omitted for `exists`).
    value: Option<String>,
}

#[derive(JsonSchema)]
#[allow(dead_code)]
struct RecallFields {
    /// Full-text query. On the simple backend this is a case-insensitive
    /// substring match; on the tantivy backend it is BM25-ranked.
    query: Option<String>,
    /// Regular-expression query matched over note content.
    regex: Option<String>,
    /// Frontmatter property filters. Requires the tantivy backend; rejected with
    /// `unsupported` on the simple backend.
    filters: Option<Vec<PropertyFilterField>>,
    /// Optional virtual-path prefix, relative to the agents folder, to filter by.
    path_prefix: Option<String>,
    /// Maximum number of hits to return (default 200, maximum 1000).
    limit: Option<u64>,
    /// Opaque pagination cursor returned by a previous call.
    cursor: Option<String>,
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
    /// The recall engine, present unless `AGENTMEM_RECALL_BACKEND=off`.
    recall: Option<Arc<RecallEngine>>,
}

impl Toolbox {
    /// Build the toolbox and precompute every tool's input schema for the active
    /// scheme. The `recall_memory_notes` tool is advertised only when `recall` is
    /// `Some` (i.e. the backend is not `off`).
    pub fn new(
        storage: Storage,
        policy: Policy,
        timezone: Tz,
        session_context_template_file: PathBuf,
        recall: Option<Arc<RecallEngine>>,
    ) -> Toolbox {
        let scheme = storage.resolver().scheme().clone();
        let tools = build_tools(&scheme, recall.is_some());
        Toolbox {
            storage,
            policy,
            timezone,
            tools,
            session_context_template_file,
            recall,
        }
    }

    /// The recall engine handle, for the server's warm-up, watcher start, and the
    /// `GET /readyz` probe.
    pub fn recall_engine(&self) -> Option<Arc<RecallEngine>> {
        self.recall.clone()
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
            "recall_memory_notes" if self.recall.is_some() => self.recall_memory_notes(args),
            _ => return None,
        };
        Some(result)
    }

    /// Notify the recall engine of the server's own write so its in-memory index
    /// updates synchronously. A no-op when recall is disabled.
    fn recall_on_write(&self, scope: &str, region: Region, physical: &PhysicalPath) {
        if let Some(engine) = &self.recall {
            engine.on_write(scope, region, physical);
        }
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

    /// Reject a generic write/edit/delete that targets an agents-folder
    /// root-level core file. Such files are reserved for the dedicated wrappers
    /// (`evolve_core_persona`, `update_task_heartbeat`); the returned error
    /// carries `path_not_permitted` and names the wrapper to use. A no-op for
    /// subfolder paths and for paths outside the agents folder.
    fn reject_if_root_reserved(&self, vpath: &VirtualPath) -> Result<(), AgentmemError> {
        if !self.storage.resolver().is_agents_root_level(vpath) {
            return Ok(());
        }
        let is_heartbeat = vpath
            .as_path()
            .file_name()
            .is_some_and(|name| name == "HEARTBEAT.md");
        let wrapper = if is_heartbeat {
            "update_task_heartbeat"
        } else {
            "evolve_core_persona"
        };
        Err(AgentmemError::RootPathReserved {
            virtual_path: vpath.as_str().to_string(),
            wrapper,
        })
    }

    /// Apply the write-side link transform to `content` for a note at `vpath`.
    /// A no-op when the scheme is empty (no scopes, hence no suffix and no
    /// cross-scope leak). May return a leak-guard `WriteDenied` when a shared
    /// note links into the caller's own scope.
    fn expand_links_for(
        &self,
        scope: &str,
        vpath: &VirtualPath,
        content: &str,
    ) -> Result<String, AgentmemError> {
        if self.scheme().is_empty() {
            return Ok(content.to_string());
        }
        let resolver = self.storage.resolver();
        let region = resolver.detect_region(vpath);
        let regions = self.policy.list_visible_regions(false);
        let index = self.storage.build_link_index(scope, &regions)?;
        crate::wikilink::expand_links(content, scope, region, resolver, &index)
    }

    /// Strip the caller's own scope suffix from link targets in `content`. A
    /// no-op when the scheme is empty.
    fn strip_links_for(&self, scope: &str, content: &str) -> String {
        if self.scheme().is_empty() {
            return content.to_string();
        }
        crate::wikilink::strip_links(content, scope, self.storage.resolver())
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
        let scope = self.resolve_scope(args, &["path_prefix", "glob", "limit", "cursor"])?;

        let path_prefix = opt_str(args, "path_prefix")?;
        let glob = match opt_str(args, "glob")? {
            Some(pattern) => Some(
                globset::Glob::new(&pattern)
                    .map_err(|e| AgentmemError::InvalidArgument {
                        message: format!("invalid glob: {e}"),
                    })?
                    .compile_matcher(),
            ),
            None => None,
        };
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

        if let Some(matcher) = &glob {
            items.retain(|p| matcher.is_match(p));
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
        let content = self.strip_links_for(&scope, &self.storage.read(&physical)?);
        let mut result = CallToolResult::success(vec![Content::text(content.clone())]);
        result.structured_content = Some(json!({ "content": content }));
        Ok(result)
    }

    fn write_memory_note(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["path", "content"])?;
        let vpath = VirtualPath::new(&require_str(args, "path")?)?;
        let content = require_str(args, "content")?;
        self.reject_if_root_reserved(&vpath)?;
        // Gate by policy before the link transform so a read-only/denied region
        // reports its policy error rather than a leak-guard `write_denied`.
        let region = self.storage.resolver().detect_region(&vpath);
        self.policy
            .gate_write(region)
            .map_err(|e| Self::policy_err(e, &vpath))?;
        let content = self.expand_links_for(&scope, &vpath, &content)?;
        self.gated_write(&scope, &vpath, |physical, storage| {
            storage.write_atomic(physical, &content)
        })
    }

    fn edit_memory_note(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["path", "search_string", "replace_string"])?;
        let vpath = VirtualPath::new(&require_str(args, "path")?)?;
        let search = require_str(args, "search_string")?;
        let replace = require_str(args, "replace_string")?;
        self.reject_if_root_reserved(&vpath)?;
        let region = self.storage.resolver().detect_region(&vpath);
        self.policy
            .gate_write(region)
            .map_err(|e| Self::policy_err(e, &vpath))?;
        // Match the on-disk (suffixed) link form: transform the search/replace
        // snippets the same way stored content was transformed on write.
        let search = self.expand_links_for(&scope, &vpath, &search)?;
        let replace = self.expand_links_for(&scope, &vpath, &replace)?;
        let resolver = self.storage.resolver();
        let physical = resolver.resolve(&scope, &vpath)?;
        if !self.storage.is_visible(&physical) {
            return Err(AgentmemError::PathNotPermitted {
                virtual_path: vpath.as_str().to_string(),
            });
        }
        let replaced = self
            .storage
            .edit_search_replace(&physical, &search, &replace)?;
        self.recall_on_write(&scope, region, &physical);
        Ok(ok_json(json!({ "chars_replaced": replaced })))
    }

    fn delete_memory_note(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["path"])?;
        let vpath = VirtualPath::new(&require_str(args, "path")?)?;
        self.reject_if_root_reserved(&vpath)?;
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
        Ok(ok_json(
            json!({ "rendered": sc.rendered, "missing": sc.missing }),
        ))
    }

    fn evolve_core_persona(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["which", "content"])?;
        let which = require_str(args, "which")?;
        // (filename, optional line cap)
        let (filename, line_cap) = match which.as_str() {
            "persona" => ("PERSONA.md", None),
            "prompt" => ("PROMPT.md", None),
            "rules" => ("RULES.md", None),
            "user" => ("USER.md", Some(100usize)),
            "memory" => ("MEMORY.md", Some(200usize)),
            other => {
                return Err(AgentmemError::InvalidArgument {
                    message: format!(
                        "which must be one of persona|prompt|rules|user|memory, got '{other}'"
                    ),
                });
            }
        };
        let content = require_str(args, "content")?;
        if let Some(cap) = line_cap {
            let lines = content.lines().count();
            if lines > cap {
                return Err(AgentmemError::InvalidArgument {
                    message: format!(
                        "{filename} must not exceed {cap} lines (got {lines}); file left unchanged"
                    ),
                });
            }
        }
        let vpath = self.agents_vpath(filename)?;
        // Expand link targets so core files (e.g. a MEMORY.md index of `[[notes]]`)
        // resolve in Obsidian; the line caps above count the agent-facing content,
        // and expansion never changes the line count.
        let content = self.expand_links_for(&scope, &vpath, &content)?;
        self.gated_write(&scope, &vpath, |physical, storage| {
            storage.write_atomic(physical, &content)
        })
    }

    fn update_task_heartbeat(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["content"])?;
        let content = require_str(args, "content")?;
        let vpath = self.agents_vpath("HEARTBEAT.md")?;
        let content = self.expand_links_for(&scope, &vpath, &content)?;
        self.gated_write(&scope, &vpath, |physical, storage| {
            storage.write_atomic(physical, &content)
        })
    }

    fn append_diary_entry(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let scope = self.resolve_scope(args, &["content", "title"])?;
        let content = require_str(args, "content")?;
        if content.is_empty() {
            return Err(AgentmemError::InvalidArgument {
                message: "content must not be empty".to_string(),
            });
        }
        let title = opt_str(args, "title")?;
        let now = Utc::now().with_timezone(&self.timezone);
        let date = now.format("%Y-%m-%d").to_string();
        let time = now.format("%H:%M:%S").to_string();
        let heading = match title.as_deref() {
            Some(t) if !t.is_empty() => format!("## {time} — {t}"),
            _ => format!("## {time}"),
        };
        let vpath = self.agents_vpath(&format!("diary/{date}.md"))?;

        let region = self.storage.resolver().detect_region(&vpath);
        self.policy
            .gate_write(region)
            .map_err(|e| Self::policy_err(e, &vpath))?;
        let content = self.expand_links_for(&scope, &vpath, &content)?;
        let resolver = self.storage.resolver();
        let physical = resolver.resolve(&scope, &vpath)?;
        if !self.storage.is_visible(&physical) {
            return Err(AgentmemError::PathNotPermitted {
                virtual_path: vpath.as_str().to_string(),
            });
        }
        let written = self
            .storage
            .read_modify_write(&physical, |current| match current {
                Some(existing) => format!("{existing}\n{heading}\n{content}\n"),
                None => format!("# {date}\n\n{heading}\n{content}\n"),
            })?;
        self.recall_on_write(&scope, region, &physical);
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
        self.recall_on_write(scope, region, &physical);
        Ok(ok_json(json!({ "bytes_written": written })))
    }

    fn recall_memory_notes(&self, args: &JsonObject) -> Result<CallToolResult, AgentmemError> {
        let engine = self
            .recall
            .as_ref()
            .expect("recall_memory_notes dispatched only when recall is enabled");
        let scope = self.resolve_scope(
            args,
            &[
                "query",
                "regex",
                "filters",
                "path_prefix",
                "limit",
                "cursor",
            ],
        )?;

        let text = opt_str(args, "query")?;
        let regex = opt_str(args, "regex")?;
        let filters = parse_filters(args)?;
        let path_prefix = opt_str(args, "path_prefix")?;
        if text.is_none() && regex.is_none() && filters.is_empty() {
            return Err(AgentmemError::InvalidArgument {
                message: "at least one of query, regex, or filters is required".to_string(),
            });
        }
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
        let query = RecallQuery {
            text,
            regex,
            filters,
            path_prefix,
            limit,
            offset,
        };
        let results = engine.recall(&scope, &regions, &query)?;

        let hits: Vec<Value> = results
            .hits
            .into_iter()
            .map(|h| json!({ "path": h.path, "score": h.score, "snippets": h.snippets }))
            .collect();
        Ok(ok_json(json!({
            "hits": hits,
            "next_cursor": results.next_cursor,
            "truncated": results.truncated,
        })))
    }
}

/// Parse the optional `filters` argument into [`PropertyFilter`]s.
fn parse_filters(args: &JsonObject) -> Result<Vec<PropertyFilter>, AgentmemError> {
    let raw = match args.get("filters") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => {
            return Err(AgentmemError::InvalidArgument {
                message: "argument 'filters' must be an array".to_string(),
            });
        }
    };
    let mut out = Vec::with_capacity(raw.len());
    for item in raw {
        let obj = item
            .as_object()
            .ok_or_else(|| AgentmemError::InvalidArgument {
                message: "each filter must be an object with 'key' and 'op'".to_string(),
            })?;
        let key = match obj.get("key") {
            Some(Value::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(AgentmemError::InvalidArgument {
                    message: "each filter must have a non-empty string 'key'".to_string(),
                });
            }
        };
        let op = match obj.get("op").and_then(Value::as_str) {
            Some("exists") => FilterOp::Exists,
            Some("eq") => FilterOp::Eq,
            Some("contains") => FilterOp::Contains,
            Some("gt") => FilterOp::Gt,
            Some("lt") => FilterOp::Lt,
            Some("ge") => FilterOp::Ge,
            Some("le") => FilterOp::Le,
            _ => {
                return Err(AgentmemError::InvalidArgument {
                    message: "filter 'op' must be one of exists|eq|contains|gt|lt|ge|le"
                        .to_string(),
                });
            }
        };
        let value = match obj.get("value") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => {
                return Err(AgentmemError::InvalidArgument {
                    message: "filter 'value' must be a string".to_string(),
                });
            }
        };
        out.push(PropertyFilter { key, op, value });
    }
    Ok(out)
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
    // `Tool` is `#[non_exhaustive]`; use its constructor rather than a struct literal.
    Tool::new(name, description, schema)
}

/// Assemble the tool list for a given scheme. The `recall_memory_notes` tool is
/// appended only when `recall_enabled`.
fn build_tools(scheme: &Scheme, recall_enabled: bool) -> Vec<Tool> {
    let mut tools = vec![
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
            "Atomically replace one foundational session file (persona|prompt|rules|user|memory) selected by `which`. Enforces caps: USER.md ≤ 100 lines, MEMORY.md ≤ 200 lines.",
            merge_schema(scheme, fields_schema::<EvolveFields>()),
        ),
        tool(
            "update_task_heartbeat",
            "Atomically replace the scope's HEARTBEAT.md.",
            merge_schema(scheme, fields_schema::<ContentOnlyFields>()),
        ),
        tool(
            "append_diary_entry",
            "Append a timestamped section to today's diary file for the active scope. Accepts an optional `title` for the entry heading.",
            merge_schema(scheme, fields_schema::<DiaryFields>()),
        ),
    ];
    if recall_enabled {
        tools.push(tool(
            "recall_memory_notes",
            "Search memory notes by content within the caller's visible set. Returns ranked hits as { path, score (0-1), snippets }. Supply at least one of `query` (full-text), `regex`, or `filters` (frontmatter properties; tantivy backend only). Paginated like list_memory_notes.",
            merge_schema(scheme, fields_schema::<RecallFields>()),
        ));
    }
    tools
}
