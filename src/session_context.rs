//! The session-context renderer: the single source of the rendered bootstrap
//! shared by the `load_session_context` tool, the `session-context` resource, and
//! the `session-context` prompt.
//!
//! It resolves the active session-context **template** through a layered lookup
//! (per-scope file → global file → compiled-in default), builds a context map
//! from the five foundational files (substituting a sentinel for any that are
//! absent), the scope keys, and a server-generated tools guide, and renders the
//! template via [`crate::template::Template`]. It never errors on absence — a
//! fresh vault renders instructions-only.

use std::collections::BTreeMap;
use std::path::Path;

use rmcp::model::Tool;

use crate::error::AgentmemError;
use crate::path::VirtualPath;
use crate::storage::Storage;
use crate::template::Template;

/// The five foundational files, paired as (placeholder leaf, filename). The
/// context key is `files.<leaf>`.
pub const FOUNDATIONAL: &[(&str, &str)] = &[
    ("persona", "PERSONA.md"),
    ("prompt", "PROMPT.md"),
    ("rules", "RULES.md"),
    ("user", "USER.md"),
    ("tools", "TOOLS.md"),
];

/// The per-scope template filename, resolved through the scope suffix mechanism
/// inside the agents folder.
const PER_SCOPE_FILE: &str = "AGENT_SESSION_CONTEXT.md";

/// Substituted for a `{{files.*}}` placeholder whose file does not exist.
const MISSING_SENTINEL: &str = "(not yet recorded — set via evolve_core_persona)";

/// The compiled-in default template, used when no per-scope or global template
/// file exists. Self-contained: a slot for each foundational file plus the
/// server-generated tools guide.
const DEFAULT_TEMPLATE: &str = "\
# Session Context

## Persona
{{files.persona}}

## Prompt
{{files.prompt}}

## Rules
{{files.rules}}

## About the User
{{files.user}}

## Tool Notes
{{files.tools}}

## Memory Tools
{{tools_guide}}
";

/// The rendered session-context plus the foundational files that were absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionContext {
    pub rendered: String,
    pub missing: Vec<String>,
}

/// Render the session-context for a validated scope map.
///
/// `scope` must contain exactly the scheme's placeholder keys (the caller
/// validates this). `global_template_file` is the configured global template
/// path (may not exist). `tools` is the live tool catalogue, used for the
/// `{{tools_guide}}` slot.
pub fn render_session_context(
    storage: &Storage,
    global_template_file: &Path,
    tools: &[Tool],
    scope: &BTreeMap<String, String>,
) -> Result<SessionContext, AgentmemError> {
    let resolver = storage.resolver();
    let rendered_scope = resolver
        .scheme()
        .render(scope)
        .map_err(|e| AgentmemError::InvalidArgument {
            message: e.to_string(),
        })?;

    // --- build the context map + missing list from foundational files ---
    let mut context: BTreeMap<String, String> = BTreeMap::new();
    let mut missing: Vec<String> = Vec::new();
    for (leaf, filename) in FOUNDATIONAL {
        let vpath = agents_vpath(storage, filename)?;
        let physical = resolver.resolve(&rendered_scope, &vpath)?;
        let key = format!("files.{leaf}");
        match storage.read(&physical) {
            Ok(content) => {
                context.insert(key, content);
            }
            Err(AgentmemError::NotFound { .. }) => {
                context.insert(key, MISSING_SENTINEL.to_string());
                missing.push((*filename).to_string());
            }
            Err(e) => return Err(e),
        }
    }

    // Scope values: `scope.<key>`.
    for (k, v) in scope {
        context.insert(format!("scope.{k}"), v.clone());
    }

    // Server-generated tools guide.
    context.insert("tools_guide".to_string(), tools_guide(tools));

    // --- resolve the template source (layered) and render ---
    let source = resolve_template_source(storage, &rendered_scope, global_template_file)?;
    let rendered = Template::parse(&source).render(&context);
    if !rendered.unknown.is_empty() {
        tracing::warn!(
            unknown = ?rendered.unknown,
            "session-context template referenced unrecognised placeholder(s); left literal"
        );
    }

    Ok(SessionContext {
        rendered: rendered.text,
        missing,
    })
}

/// Resolve the active template source: per-scope file → global file → default.
/// Absence at any layer is non-fatal; genuine IO errors propagate.
fn resolve_template_source(
    storage: &Storage,
    rendered_scope: &str,
    global_template_file: &Path,
) -> Result<String, AgentmemError> {
    // (1) per-scope file, via the scope suffix mechanism inside the agents folder.
    let vpath = agents_vpath(storage, PER_SCOPE_FILE)?;
    let physical = storage.resolver().resolve(rendered_scope, &vpath)?;
    match storage.read(&physical) {
        Ok(content) => return Ok(content),
        Err(AgentmemError::NotFound { .. }) => {}
        Err(e) => return Err(e),
    }

    // (2) global template file (an absolute/arbitrary path; read directly).
    match std::fs::read_to_string(global_template_file) {
        Ok(content) => return Ok(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(AgentmemError::io("reading session-context template", &e)),
    }

    // (3) compiled-in default.
    Ok(DEFAULT_TEMPLATE.to_string())
}

/// The clean virtual path of a conventional file relative to the agents folder
/// (e.g. `Agents/PERSONA.md`, or `PERSONA.md` when the agents folder is the
/// vault root).
fn agents_vpath(storage: &Storage, relative: &str) -> Result<VirtualPath, AgentmemError> {
    let agents = storage.resolver().agents_dir();
    let full = if agents.as_str().is_empty() {
        relative.to_string()
    } else {
        format!("{agents}/{relative}")
    };
    VirtualPath::new(&full)
}

/// Build the memory-tools guide from the live tool catalogue.
fn tools_guide(tools: &[Tool]) -> String {
    let mut out = String::from(
        "These memory tools are available. Every call must carry the scope keys \
         defined by the server's VFS scheme.\n\n",
    );
    for tool in tools {
        let desc = tool.description.as_deref().unwrap_or("");
        out.push_str(&format!("- `{}`: {}\n", tool.name, desc));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::PathResolver;
    use crate::scheme::Scheme;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;

    fn storage_for(tmp: &TempDir, scheme: &str) -> Storage {
        let resolver = PathResolver::new(
            tmp.path().canonicalize().unwrap(),
            Utf8PathBuf::from("Agents"),
            Scheme::parse(scheme).unwrap(),
        );
        Storage::new(resolver, true, false)
    }

    fn scope(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn write(tmp: &TempDir, rel: &str, content: &str) {
        let path = tmp.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// No files, no template → compiled-in default with all sentinels; all five
    /// foundational files reported missing.
    #[test]
    fn empty_vault_renders_default_with_sentinels() {
        let tmp = TempDir::new().unwrap();
        let storage = storage_for(&tmp, "<agent>.<user>");
        let global = tmp.path().join("AGENT_SESSION_CONTEXT.md");
        let sc =
            render_session_context(&storage, &global, &[], &scope(&[("agent", "c"), ("user", "a")]))
                .unwrap();
        assert!(sc.rendered.contains(MISSING_SENTINEL));
        assert_eq!(sc.missing.len(), 5);
        assert!(sc.rendered.contains("# Session Context"));
    }

    /// Foundational files present are substituted; absent ones get the sentinel
    /// and are listed in `missing`.
    #[test]
    fn substitutes_present_files_and_sentinels_absent() {
        let tmp = TempDir::new().unwrap();
        let storage = storage_for(&tmp, "<agent>.<user>");
        write(&tmp, "Agents/c.a/PERSONA.c.a.md", "PERSONA-BODY");
        write(&tmp, "Agents/c.a/RULES.c.a.md", "RULES-BODY");
        let global = tmp.path().join("AGENT_SESSION_CONTEXT.md");
        let sc =
            render_session_context(&storage, &global, &[], &scope(&[("agent", "c"), ("user", "a")]))
                .unwrap();
        assert!(sc.rendered.contains("PERSONA-BODY"));
        assert!(sc.rendered.contains("RULES-BODY"));
        assert!(sc.rendered.contains(MISSING_SENTINEL));
        assert_eq!(
            sc.missing,
            vec!["PROMPT.md".to_string(), "USER.md".to_string(), "TOOLS.md".to_string()]
        );
    }

    /// `{{files.user}}` (file contents) and `{{scope.user}}` (scope value) are
    /// distinct.
    #[test]
    fn file_and_scope_namespaces_are_distinct() {
        let tmp = TempDir::new().unwrap();
        let storage = storage_for(&tmp, "<agent>.<user>");
        write(&tmp, "Agents/c.alice/USER.c.alice.md", "USER-FILE-BODY");
        let global = tmp.path().join("missing.md");
        // Per-scope template exercising both namespaces.
        write(
            &tmp,
            "Agents/c.alice/AGENT_SESSION_CONTEXT.c.alice.md",
            "file={{files.user}} scope={{scope.user}}",
        );
        let sc = render_session_context(
            &storage,
            &global,
            &[],
            &scope(&[("agent", "c"), ("user", "alice")]),
        )
        .unwrap();
        assert_eq!(sc.rendered, "file=USER-FILE-BODY scope=alice");
    }

    /// Per-scope template wins over the global file, which wins over the default.
    #[test]
    fn layered_resolution_prefers_per_scope_then_global_then_default() {
        let tmp = TempDir::new().unwrap();
        let storage = storage_for(&tmp, "<agent>.<user>");
        let global = tmp.path().join("GLOBAL.md");

        // Only default available.
        let sc = render_session_context(&storage, &global, &[], &scope(&[("agent", "c"), ("user", "a")]))
            .unwrap();
        assert!(sc.rendered.contains("# Session Context"));

        // Global present → used.
        std::fs::write(&global, "GLOBAL-TEMPLATE").unwrap();
        let sc = render_session_context(&storage, &global, &[], &scope(&[("agent", "c"), ("user", "a")]))
            .unwrap();
        assert_eq!(sc.rendered, "GLOBAL-TEMPLATE");

        // Per-scope present → overrides global.
        write(&tmp, "Agents/c.a/AGENT_SESSION_CONTEXT.c.a.md", "PER-SCOPE-TEMPLATE");
        let sc = render_session_context(&storage, &global, &[], &scope(&[("agent", "c"), ("user", "a")]))
            .unwrap();
        assert_eq!(sc.rendered, "PER-SCOPE-TEMPLATE");
    }
}
