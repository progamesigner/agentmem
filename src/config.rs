//! Environment-variable-driven configuration with optional CLI overrides.
//!
//! The canonical configuration surface is the environment; CLI flags (parsed by
//! [`Cli`]) override the matching variable. Every variable except
//! `AGENTMEM_ROOT_DIR` has a default, and invalid values fail fast with a
//! human-readable message naming the offending variable.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use camino::Utf8PathBuf;
use chrono_tz::Tz;

use crate::error::AgentmemError;
use crate::policy::Policy;
use crate::scheme::Scheme;

/// The default agents-folder name.
pub const DEFAULT_AGENTS_DIR: &str = "Agents";
/// The default VFS suffix scheme.
pub const DEFAULT_SCHEME: &str = "<agent>.<user>";
/// The default session-context template filename, relative to the vault root.
pub const DEFAULT_SESSION_CONTEXT_FILE: &str = "AGENT_SESSION_CONTEXT.md";
/// The default HTTP bind address.
pub const DEFAULT_HTTP_BIND: &str = "127.0.0.1:8000";
/// The default tracing filter directive.
pub const DEFAULT_LOG_FILTER: &str = "warn,agentmem=info";

/// Default filesystem-watcher debounce window, in milliseconds.
pub const DEFAULT_RECALL_WATCH_DEBOUNCE_MS: u64 = 500;
/// Default upper bound on bytes scanned by a regex-only recall query before the
/// scan is truncated and the result flagged.
pub const DEFAULT_RECALL_REGEX_SCAN_BYTES: usize = 64 * 1024 * 1024;
/// Default cap on the number of resident per-scope indexes before idle ones are
/// evicted (they rebuild on next access).
pub const DEFAULT_RECALL_MAX_RESIDENT_SCOPES: usize = 256;
/// Default freshness window: a recall reuses its index without a stat-diff
/// reconcile for this long after the last one (unless the watcher marks it dirty).
pub const DEFAULT_RECALL_FRESHNESS_MS: u64 = 2000;

const VAR_ROOT_DIR: &str = "AGENTMEM_ROOT_DIR";
const VAR_AGENTS_DIR: &str = "AGENTMEM_AGENTS_DIR";
const VAR_SCHEME: &str = "AGENTMEM_VFS_SCHEME";
const VAR_SESSION_CONTEXT_TEMPLATE_FILE: &str = "AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE";
const VAR_POLICY: &str = "AGENTMEM_POLICY";
const VAR_TRANSPORT: &str = "AGENTMEM_TRANSPORT";
const VAR_HTTP_BIND: &str = "AGENTMEM_HTTP_BIND";
const VAR_HTTP_BEARER: &str = "AGENTMEM_HTTP_BEARER";
const VAR_HTTP_ALLOWED_HOSTS: &str = "AGENTMEM_HTTP_ALLOWED_HOSTS";
const VAR_TIMEZONE: &str = "AGENTMEM_TIMEZONE";
const VAR_HONOR_IGNORE: &str = "AGENTMEM_HONOR_IGNORE_FILES";
const VAR_INCLUDE_HIDDEN: &str = "AGENTMEM_INCLUDE_HIDDEN";
const VAR_INCLUDE_HIDDEN_GLOBS: &str = "AGENTMEM_INCLUDE_HIDDEN_GLOBS";
const VAR_LOG: &str = "AGENTMEM_LOG";
const VAR_RECALL_BACKEND: &str = "AGENTMEM_RECALL_BACKEND";
const VAR_RECALL_WATCH_DEBOUNCE_MS: &str = "AGENTMEM_RECALL_WATCH_DEBOUNCE_MS";
const VAR_RECALL_REGEX_SCAN_BYTES: &str = "AGENTMEM_RECALL_REGEX_SCAN_BYTES";
const VAR_RECALL_MAX_RESIDENT_SCOPES: &str = "AGENTMEM_RECALL_MAX_RESIDENT_SCOPES";
const VAR_RECALL_FRESHNESS_MS: &str = "AGENTMEM_RECALL_FRESHNESS_MS";

/// The selected transport and its parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Stdio,
    Http {
        bind: SocketAddr,
        bearer: Option<String>,
        /// `Host`-header allow-list for the Streamable HTTP transport. Empty
        /// means "use rmcp's loopback-only default"; the sole entry `*` disables
        /// `Host` validation entirely.
        allowed_hosts: Vec<String>,
    },
}

impl Transport {
    /// `true` when bound on a non-loopback interface without a bearer token —
    /// the condition that warrants a startup warning.
    pub fn is_unauthenticated_non_loopback(&self) -> bool {
        match self {
            Transport::Http { bind, bearer, .. } => bearer.is_none() && !bind.ip().is_loopback(),
            Transport::Stdio => false,
        }
    }
}

/// The requested recall search backend.
///
/// `Tantivy` is honoured only when the binary is built with the `recall-tantivy`
/// cargo feature; otherwise the engine falls back to `Simple` (see
/// `crate::recall`). `Off` suppresses the `recall_memory_notes` tool entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallBackendKind {
    Simple,
    Tantivy,
    Off,
}

impl RecallBackendKind {
    /// The accepted variable values, for error messages.
    pub const ACCEPTED: &'static [&'static str] = &["simple", "tantivy", "off"];

    /// Parse the `AGENTMEM_RECALL_BACKEND` value.
    pub fn parse(s: &str) -> Option<RecallBackendKind> {
        match s {
            "simple" => Some(RecallBackendKind::Simple),
            "tantivy" => Some(RecallBackendKind::Tantivy),
            "off" => Some(RecallBackendKind::Off),
            _ => None,
        }
    }

    /// The canonical string form (matches the accepted variable value).
    pub fn as_str(self) -> &'static str {
        match self {
            RecallBackendKind::Simple => "simple",
            RecallBackendKind::Tantivy => "tantivy",
            RecallBackendKind::Off => "off",
        }
    }
}

/// Recall search configuration: the requested backend plus its tuning knobs.
#[derive(Debug, Clone)]
pub struct RecallConfig {
    /// The requested backend (the effective backend is resolved at engine build
    /// time against the `recall-tantivy` feature).
    pub backend: RecallBackendKind,
    /// Filesystem-watcher debounce window.
    pub watch_debounce: Duration,
    /// Upper bound on bytes a regex-only query scans before truncating.
    pub regex_scan_byte_cap: usize,
    /// Cap on resident per-scope indexes before idle eviction.
    pub max_resident_scopes: usize,
    /// How long an index stays fresh before a query re-runs the stat-diff
    /// reconcile (the watcher can mark it dirty sooner).
    pub freshness: Duration,
}

/// The fully-resolved server configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Canonical absolute vault root.
    pub root_dir: PathBuf,
    /// Agents folder relative to the root; empty means "the agents folder is the
    /// vault root".
    pub agents_dir: Utf8PathBuf,
    pub scheme: Scheme,
    /// Absolute path to the global session-context template file (may not exist).
    pub session_context_template_file: PathBuf,
    pub policy: Policy,
    pub transport: Transport,
    pub timezone: Tz,
    pub honor_ignore_files: bool,
    pub include_hidden: bool,
    /// Gitignore-style glob patterns (relative to the vault root) whose matches —
    /// and their whole subtree — are exempted from hidden-segment filtering.
    /// Empty by default. Validated at build time.
    pub include_hidden_globs: Vec<String>,
    /// The `tracing_subscriber::EnvFilter` directive string.
    pub log_filter: String,
    /// Recall search configuration.
    pub recall: RecallConfig,
}

/// CLI flags that mirror — and override — the environment variables.
#[derive(Debug, Default, clap::Parser)]
#[command(
    name = "agentmem",
    version,
    about = "MCP server for multi-tenant agent memory"
)]
pub struct Cli {
    /// Vault root directory (overrides AGENTMEM_ROOT_DIR).
    #[arg(long)]
    pub root_dir: Option<PathBuf>,
    /// Agents folder name (overrides AGENTMEM_AGENTS_DIR).
    #[arg(long)]
    pub agents_dir: Option<String>,
    /// VFS suffix scheme (overrides AGENTMEM_VFS_SCHEME).
    #[arg(long)]
    pub vfs_scheme: Option<String>,
    /// Global session-context template file (overrides AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE).
    #[arg(long)]
    pub session_context_template_file: Option<PathBuf>,
    /// Policy: scoped|namespaced|readonly|readwrite (overrides AGENTMEM_POLICY).
    #[arg(long)]
    pub policy: Option<String>,
    /// Transport: stdio|http (overrides AGENTMEM_TRANSPORT).
    #[arg(long)]
    pub transport: Option<String>,
    /// HTTP bind address (overrides AGENTMEM_HTTP_BIND).
    #[arg(long)]
    pub http_bind: Option<String>,
    /// HTTP bearer token (overrides AGENTMEM_HTTP_BEARER).
    #[arg(long)]
    pub http_bearer: Option<String>,
    /// Comma-separated `Host` allow-list for the HTTP transport; `*` disables
    /// validation (overrides AGENTMEM_HTTP_ALLOWED_HOSTS).
    #[arg(long)]
    pub http_allowed_hosts: Option<String>,
    /// IANA timezone (overrides AGENTMEM_TIMEZONE).
    #[arg(long)]
    pub timezone: Option<String>,
    /// Honour .gitignore/.obsidianignore (overrides AGENTMEM_HONOR_IGNORE_FILES).
    #[arg(long)]
    pub honor_ignore_files: Option<bool>,
    /// Include hidden dotfiles (overrides AGENTMEM_INCLUDE_HIDDEN).
    #[arg(long)]
    pub include_hidden: Option<bool>,
    /// Comma-separated globs whose matches are exempt from hidden filtering
    /// (overrides AGENTMEM_INCLUDE_HIDDEN_GLOBS).
    #[arg(long)]
    pub include_hidden_globs: Option<String>,
    /// Tracing filter directive (overrides AGENTMEM_LOG).
    #[arg(long)]
    pub log: Option<String>,
    /// Recall backend: simple|tantivy|off (overrides AGENTMEM_RECALL_BACKEND).
    #[arg(long)]
    pub recall_backend: Option<String>,
    /// Print the effective configuration to stderr and exit.
    #[arg(long)]
    pub print_config: bool,
}

impl Cli {
    /// The flags that are set, keyed by the env var they override.
    fn as_overrides(&self) -> HashMap<&'static str, String> {
        let mut m = HashMap::new();
        if let Some(v) = &self.root_dir {
            m.insert(VAR_ROOT_DIR, v.to_string_lossy().into_owned());
        }
        if let Some(v) = &self.agents_dir {
            m.insert(VAR_AGENTS_DIR, v.clone());
        }
        if let Some(v) = &self.vfs_scheme {
            m.insert(VAR_SCHEME, v.clone());
        }
        if let Some(v) = &self.session_context_template_file {
            m.insert(
                VAR_SESSION_CONTEXT_TEMPLATE_FILE,
                v.to_string_lossy().into_owned(),
            );
        }
        if let Some(v) = &self.policy {
            m.insert(VAR_POLICY, v.clone());
        }
        if let Some(v) = &self.transport {
            m.insert(VAR_TRANSPORT, v.clone());
        }
        if let Some(v) = &self.http_bind {
            m.insert(VAR_HTTP_BIND, v.clone());
        }
        if let Some(v) = &self.http_bearer {
            m.insert(VAR_HTTP_BEARER, v.clone());
        }
        if let Some(v) = &self.http_allowed_hosts {
            m.insert(VAR_HTTP_ALLOWED_HOSTS, v.clone());
        }
        if let Some(v) = &self.timezone {
            m.insert(VAR_TIMEZONE, v.clone());
        }
        if let Some(v) = &self.honor_ignore_files {
            m.insert(VAR_HONOR_IGNORE, v.to_string());
        }
        if let Some(v) = &self.include_hidden {
            m.insert(VAR_INCLUDE_HIDDEN, v.to_string());
        }
        if let Some(v) = &self.include_hidden_globs {
            m.insert(VAR_INCLUDE_HIDDEN_GLOBS, v.clone());
        }
        if let Some(v) = &self.log {
            m.insert(VAR_LOG, v.clone());
        }
        if let Some(v) = &self.recall_backend {
            m.insert(VAR_RECALL_BACKEND, v.clone());
        }
        m
    }
}

fn config_err(message: impl Into<String>) -> AgentmemError {
    AgentmemError::Config {
        message: message.into(),
    }
}

impl Config {
    /// Build configuration from the process environment.
    pub fn from_env() -> Result<Config, AgentmemError> {
        Config::build(&|k| std::env::var(k).ok())
    }

    /// Build configuration from the process environment, with CLI flags taking
    /// precedence over the matching variable.
    pub fn from_cli_and_env(cli: &Cli) -> Result<Config, AgentmemError> {
        let overrides = cli.as_overrides();
        Config::build(&|k| overrides.get(k).cloned().or_else(|| std::env::var(k).ok()))
    }

    /// Core builder over an arbitrary variable getter (used by tests too).
    fn build(get: &dyn Fn(&str) -> Option<String>) -> Result<Config, AgentmemError> {
        // --- root dir (required) ---
        let root_raw = get(VAR_ROOT_DIR)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| config_err(format!("{VAR_ROOT_DIR} is required but was not set")))?;
        let root_path = PathBuf::from(&root_raw);
        let metadata = std::fs::metadata(&root_path).map_err(|_| {
            config_err(format!(
                "{VAR_ROOT_DIR} does not exist or is not accessible: {root_raw}"
            ))
        })?;
        if !metadata.is_dir() {
            return Err(config_err(format!(
                "{VAR_ROOT_DIR} is not a directory: {root_raw}"
            )));
        }
        let root_dir = root_path.canonicalize().map_err(|_| {
            config_err(format!(
                "{VAR_ROOT_DIR} could not be canonicalised: {root_raw}"
            ))
        })?;

        // --- agents dir ---
        let agents_raw = get(VAR_AGENTS_DIR).unwrap_or_else(|| DEFAULT_AGENTS_DIR.to_string());
        let agents_dir = parse_agents_dir(&agents_raw)?;

        // --- scheme ---
        let scheme_raw = get(VAR_SCHEME).unwrap_or_else(|| DEFAULT_SCHEME.to_string());
        let scheme = Scheme::parse(&scheme_raw)
            .map_err(|e| config_err(format!("{VAR_SCHEME} is invalid ({scheme_raw:?}): {e}")))?;

        // --- session-context template file ---
        // A relative path is resolved against the vault root; the default is
        // `<root>/AGENT_SESSION_CONTEXT.md`. The file need not exist.
        let session_context_template_file =
            match get(VAR_SESSION_CONTEXT_TEMPLATE_FILE).filter(|s| !s.is_empty()) {
                Some(raw) => {
                    let p = PathBuf::from(raw);
                    if p.is_absolute() { p } else { root_dir.join(p) }
                }
                None => root_dir.join(DEFAULT_SESSION_CONTEXT_FILE),
            };

        // --- policy ---
        let policy_raw = get(VAR_POLICY).unwrap_or_else(|| "namespaced".to_string());
        let policy = Policy::parse(&policy_raw).ok_or_else(|| {
            config_err(format!(
                "{VAR_POLICY} must be one of {:?}, got {policy_raw:?}",
                Policy::ACCEPTED
            ))
        })?;

        // --- transport ---
        let transport = parse_transport(get)?;

        // --- timezone ---
        let tz_raw = get(VAR_TIMEZONE).unwrap_or_else(|| "UTC".to_string());
        let timezone: Tz = tz_raw.parse().map_err(|_| {
            config_err(format!(
                "{VAR_TIMEZONE} is not a valid IANA timezone: {tz_raw}"
            ))
        })?;

        // --- visibility flags ---
        let honor_ignore_files = parse_bool(get, VAR_HONOR_IGNORE, true)?;
        let include_hidden = parse_bool(get, VAR_INCLUDE_HIDDEN, false)?;
        // Comma-separated globs; trim, drop empties, then validate by compiling.
        let include_hidden_globs: Vec<String> = get(VAR_INCLUDE_HIDDEN_GLOBS)
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        crate::storage::compile_include_globs(&root_dir, &include_hidden_globs).map_err(|e| {
            config_err(format!(
                "{VAR_INCLUDE_HIDDEN_GLOBS} contains an invalid glob pattern: {e}"
            ))
        })?;

        // --- logging ---
        let log_filter = get(VAR_LOG)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_string());

        // --- recall ---
        let recall = parse_recall(get)?;

        Ok(Config {
            root_dir,
            agents_dir,
            scheme,
            session_context_template_file,
            policy,
            transport,
            timezone,
            honor_ignore_files,
            include_hidden,
            include_hidden_globs,
            log_filter,
            recall,
        })
    }

    /// A [`crate::path::PathResolver`] for this configuration.
    pub fn resolver(&self) -> crate::path::PathResolver {
        crate::path::PathResolver::new(
            self.root_dir.clone(),
            self.agents_dir.clone(),
            self.scheme.clone(),
        )
    }

    /// A human-readable multi-line summary for `--print-config`.
    pub fn describe(&self) -> String {
        let transport = match &self.transport {
            Transport::Stdio => "stdio".to_string(),
            Transport::Http {
                bind,
                bearer,
                allowed_hosts,
            } => {
                let hosts = describe_allowed_hosts(allowed_hosts);
                format!(
                    "http bind={bind} bearer={} allowed_hosts={hosts}",
                    if bearer.is_some() { "set" } else { "unset" }
                )
            }
        };
        format!(
            "root_dir = {root}\n\
             agents_dir = {agents}\n\
             scheme = {scheme:?}\n\
             session_context_template_file = {sctf}\n\
             policy = {policy:?}\n\
             transport = {transport}\n\
             timezone = {tz}\n\
             honor_ignore_files = {ignore}\n\
             include_hidden = {hidden}\n\
             include_hidden_globs = {hidden_globs}\n\
             log_filter = {log}\n\
             recall_backend = {recall}",
            root = self.root_dir.display(),
            agents = if self.agents_dir.as_str().is_empty() {
                "<vault root>"
            } else {
                self.agents_dir.as_str()
            },
            scheme = self.scheme,
            sctf = self.session_context_template_file.display(),
            policy = self.policy,
            transport = transport,
            tz = self.timezone,
            ignore = self.honor_ignore_files,
            hidden = self.include_hidden,
            hidden_globs = if self.include_hidden_globs.is_empty() {
                "<none>".to_string()
            } else {
                self.include_hidden_globs.join(", ")
            },
            log = self.log_filter,
            recall = self.recall.backend.as_str(),
        )
    }

    #[cfg(test)]
    fn from_pairs(pairs: &[(&str, &str)]) -> Result<Config, AgentmemError> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::build(&|k| map.get(k).cloned())
    }
}

/// Parse the agents-folder name. `.` or empty means "the vault root"; otherwise
/// it must be a relative path with no traversal.
fn parse_agents_dir(raw: &str) -> Result<Utf8PathBuf, AgentmemError> {
    if raw.is_empty() || raw == "." {
        return Ok(Utf8PathBuf::new());
    }
    let path = Utf8PathBuf::from(raw);
    if path.is_absolute() {
        return Err(config_err(format!(
            "{VAR_AGENTS_DIR} must be relative, got {raw:?}"
        )));
    }
    for component in path.components() {
        use camino::Utf8Component::*;
        match component {
            Normal(_) | CurDir => {}
            ParentDir | RootDir | Prefix(_) => {
                return Err(config_err(format!(
                    "{VAR_AGENTS_DIR} must not contain traversal, got {raw:?}"
                )));
            }
        }
    }
    Ok(path)
}

fn parse_transport(get: &dyn Fn(&str) -> Option<String>) -> Result<Transport, AgentmemError> {
    let kind = get(VAR_TRANSPORT).unwrap_or_else(|| "http".to_string());
    match kind.as_str() {
        "stdio" => Ok(Transport::Stdio),
        "http" => {
            let bind_raw = get(VAR_HTTP_BIND)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_HTTP_BIND.to_string());
            let bind: SocketAddr = bind_raw.parse().map_err(|_| {
                config_err(format!(
                    "{VAR_HTTP_BIND} is not a valid socket address: {bind_raw}"
                ))
            })?;
            let bearer = get(VAR_HTTP_BEARER).filter(|s| !s.is_empty());
            // Comma-separated `Host` allow-list; trim and drop empties. An empty
            // result leaves rmcp's loopback-only default in place; a sole `*`
            // disables `Host` validation.
            let allowed_hosts: Vec<String> = get(VAR_HTTP_ALLOWED_HOSTS)
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(Transport::Http {
                bind,
                bearer,
                allowed_hosts,
            })
        }
        other => Err(config_err(format!(
            "{VAR_TRANSPORT} must be one of [\"stdio\", \"http\"], got {other:?}"
        ))),
    }
}

/// Parse the recall configuration block. The backend value is validated here; the
/// effective backend (honouring the `recall-tantivy` feature) is resolved later by
/// the engine. Tuning knobs fall back to their documented defaults.
fn parse_recall(get: &dyn Fn(&str) -> Option<String>) -> Result<RecallConfig, AgentmemError> {
    let backend = match get(VAR_RECALL_BACKEND).filter(|s| !s.is_empty()) {
        Some(raw) => RecallBackendKind::parse(&raw).ok_or_else(|| {
            config_err(format!(
                "{VAR_RECALL_BACKEND} must be one of {:?}, got {raw:?}",
                RecallBackendKind::ACCEPTED
            ))
        })?,
        None => RecallBackendKind::Simple,
    };
    let watch_debounce = Duration::from_millis(parse_u64(
        get,
        VAR_RECALL_WATCH_DEBOUNCE_MS,
        DEFAULT_RECALL_WATCH_DEBOUNCE_MS,
    )?);
    let regex_scan_byte_cap = parse_u64(
        get,
        VAR_RECALL_REGEX_SCAN_BYTES,
        DEFAULT_RECALL_REGEX_SCAN_BYTES as u64,
    )? as usize;
    let max_resident_scopes = parse_u64(
        get,
        VAR_RECALL_MAX_RESIDENT_SCOPES,
        DEFAULT_RECALL_MAX_RESIDENT_SCOPES as u64,
    )? as usize;
    let freshness = Duration::from_millis(parse_u64(
        get,
        VAR_RECALL_FRESHNESS_MS,
        DEFAULT_RECALL_FRESHNESS_MS,
    )?);
    Ok(RecallConfig {
        backend,
        watch_debounce,
        regex_scan_byte_cap,
        max_resident_scopes,
        freshness,
    })
}

fn parse_u64(
    get: &dyn Fn(&str) -> Option<String>,
    var: &str,
    default: u64,
) -> Result<u64, AgentmemError> {
    match get(var).filter(|s| !s.is_empty()) {
        None => Ok(default),
        Some(v) => v
            .parse::<u64>()
            .map_err(|_| config_err(format!("{var} must be a non-negative integer, got {v:?}"))),
    }
}

fn parse_bool(
    get: &dyn Fn(&str) -> Option<String>,
    var: &str,
    default: bool,
) -> Result<bool, AgentmemError> {
    match get(var) {
        None => Ok(default),
        Some(v) => match v.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            other => Err(config_err(format!(
                "{var} must be \"true\" or \"false\", got {other:?}"
            ))),
        },
    }
}

/// A human-readable rendering of the resolved `Host` allow-list for
/// `--print-config`: `<loopback default>` when empty, `<validation disabled>`
/// for the `*` sentinel, otherwise the comma-joined entries.
fn describe_allowed_hosts(allowed_hosts: &[String]) -> String {
    if allowed_hosts.is_empty() {
        "<loopback default>".to_string()
    } else if allowed_hosts == ["*"] {
        "<validation disabled>".to_string()
    } else {
        allowed_hosts.join(", ")
    }
}

/// `127.0.0.1:8000` as a [`SocketAddr`], for callers that need the default.
pub fn default_http_bind() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, 8000))
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;

    fn with_root<'a>(tmp: &'a TempDir, extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, String)> {
        let mut v: Vec<(&str, String)> =
            vec![(VAR_ROOT_DIR, tmp.path().to_string_lossy().into_owned())];
        for (k, val) in extra {
            v.push((k, val.to_string()));
        }
        v
    }

    fn build(pairs: Vec<(&str, String)>) -> Result<Config, AgentmemError> {
        let map: HashMap<String, String> =
            pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Config::build(&|k| map.get(k).cloned())
    }

    #[test]
    fn missing_root_dir_fails() {
        let err = Config::from_pairs(&[]).unwrap_err();
        assert!(err.to_string().contains(VAR_ROOT_DIR));
    }

    #[test]
    fn root_dir_not_a_directory_fails() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("a-file");
        std::fs::write(&file, b"x").unwrap();
        let err = Config::from_pairs(&[(VAR_ROOT_DIR, file.to_str().unwrap())]).unwrap_err();
        assert!(err.to_string().contains("not a directory"));
    }

    #[test]
    fn all_defaults_apply() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[])).unwrap();
        assert_eq!(cfg.agents_dir.as_str(), "Agents");
        assert_eq!(cfg.scheme, Scheme::parse("<agent>.<user>").unwrap());
        assert_eq!(cfg.policy, Policy::Namespaced);
        assert_eq!(
            cfg.transport,
            Transport::Http {
                bind: default_http_bind(),
                bearer: None,
                allowed_hosts: vec![],
            }
        );
        assert_eq!(cfg.timezone, Tz::UTC);
        assert!(cfg.honor_ignore_files);
        assert!(!cfg.include_hidden);
        assert_eq!(cfg.log_filter, DEFAULT_LOG_FILTER);
        // Recall defaults: simple backend, documented tuning knobs.
        assert_eq!(cfg.recall.backend, RecallBackendKind::Simple);
        assert_eq!(
            cfg.recall.watch_debounce,
            Duration::from_millis(DEFAULT_RECALL_WATCH_DEBOUNCE_MS)
        );
        assert_eq!(
            cfg.recall.regex_scan_byte_cap,
            DEFAULT_RECALL_REGEX_SCAN_BYTES
        );
        assert_eq!(
            cfg.recall.max_resident_scopes,
            DEFAULT_RECALL_MAX_RESIDENT_SCOPES
        );
    }

    #[test]
    fn recall_backend_parsed_and_validated() {
        let tmp = TempDir::new().unwrap();
        for (raw, want) in [
            ("simple", RecallBackendKind::Simple),
            ("tantivy", RecallBackendKind::Tantivy),
            ("off", RecallBackendKind::Off),
        ] {
            let cfg = build(with_root(&tmp, &[(VAR_RECALL_BACKEND, raw)])).unwrap();
            assert_eq!(cfg.recall.backend, want);
            assert_eq!(cfg.recall.backend.as_str(), raw);
        }
        let err = build(with_root(&tmp, &[(VAR_RECALL_BACKEND, "fuzzy")])).unwrap_err();
        assert!(err.to_string().contains(VAR_RECALL_BACKEND));
    }

    #[test]
    fn recall_tuning_knobs_override_and_reject_garbage() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(
            &tmp,
            &[
                (VAR_RECALL_WATCH_DEBOUNCE_MS, "250"),
                (VAR_RECALL_REGEX_SCAN_BYTES, "1024"),
                (VAR_RECALL_MAX_RESIDENT_SCOPES, "8"),
                (VAR_RECALL_FRESHNESS_MS, "100"),
            ],
        ))
        .unwrap();
        assert_eq!(cfg.recall.watch_debounce, Duration::from_millis(250));
        assert_eq!(cfg.recall.regex_scan_byte_cap, 1024);
        assert_eq!(cfg.recall.max_resident_scopes, 8);
        assert_eq!(cfg.recall.freshness, Duration::from_millis(100));

        let err = build(with_root(&tmp, &[(VAR_RECALL_REGEX_SCAN_BYTES, "lots")])).unwrap_err();
        assert!(err.to_string().contains(VAR_RECALL_REGEX_SCAN_BYTES));
    }

    #[test]
    fn session_context_template_file_default_and_override() {
        let tmp = TempDir::new().unwrap();
        // Default: <root>/AGENT_SESSION_CONTEXT.md
        let cfg = build(with_root(&tmp, &[])).unwrap();
        assert_eq!(
            cfg.session_context_template_file,
            tmp.path()
                .canonicalize()
                .unwrap()
                .join("AGENT_SESSION_CONTEXT.md")
        );
        // Relative override resolves against the vault root.
        let cfg = build(with_root(
            &tmp,
            &[(VAR_SESSION_CONTEXT_TEMPLATE_FILE, "custom/bootstrap.md")],
        ))
        .unwrap();
        assert_eq!(
            cfg.session_context_template_file,
            tmp.path()
                .canonicalize()
                .unwrap()
                .join("custom/bootstrap.md")
        );
        // Absolute override is used as-is.
        let cfg = build(with_root(
            &tmp,
            &[(
                VAR_SESSION_CONTEXT_TEMPLATE_FILE,
                "/etc/agentmem/bootstrap.md",
            )],
        ))
        .unwrap();
        assert_eq!(
            cfg.session_context_template_file,
            PathBuf::from("/etc/agentmem/bootstrap.md")
        );
    }

    #[test]
    fn vault_root_as_agents_folder() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[(VAR_AGENTS_DIR, ".")])).unwrap();
        assert_eq!(cfg.agents_dir.as_str(), "");
    }

    #[test]
    fn agents_dir_traversal_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let err = build(with_root(&tmp, &[(VAR_AGENTS_DIR, "../escape")])).unwrap_err();
        assert!(err.to_string().contains(VAR_AGENTS_DIR));
    }

    #[test]
    fn malformed_scheme_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let err = build(with_root(&tmp, &[(VAR_SCHEME, "<agent")])).unwrap_err();
        assert!(err.to_string().contains(VAR_SCHEME));
    }

    #[test]
    fn invalid_policy_lists_accepted_values() {
        let tmp = TempDir::new().unwrap();
        let err = build(with_root(&tmp, &[(VAR_POLICY, "bogus")])).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("scoped") && msg.contains("readwrite"));
    }

    #[test]
    fn invalid_timezone_fails_fast() {
        let tmp = TempDir::new().unwrap();
        let err = build(with_root(&tmp, &[(VAR_TIMEZONE, "Mars/Olympus")])).unwrap_err();
        assert!(err.to_string().contains(VAR_TIMEZONE));
    }

    #[test]
    fn custom_timezone_parses() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[(VAR_TIMEZONE, "Asia/Taipei")])).unwrap();
        assert_eq!(cfg.timezone, Tz::Asia__Taipei);
    }

    #[test]
    fn invalid_boolean_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let err = build(with_root(&tmp, &[(VAR_INCLUDE_HIDDEN, "yes")])).unwrap_err();
        assert!(err.to_string().contains(VAR_INCLUDE_HIDDEN));
    }

    #[test]
    fn stdio_ignores_http_bind() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(
            &tmp,
            &[(VAR_TRANSPORT, "stdio"), (VAR_HTTP_BIND, "0.0.0.0:1234")],
        ))
        .unwrap();
        assert_eq!(cfg.transport, Transport::Stdio);
    }

    #[test]
    fn http_bind_and_bearer_parse() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(
            &tmp,
            &[(VAR_HTTP_BIND, "0.0.0.0:9000"), (VAR_HTTP_BEARER, "secret")],
        ))
        .unwrap();
        // Non-loopback but authenticated → not flagged.
        assert!(!cfg.transport.is_unauthenticated_non_loopback());
        match cfg.transport {
            Transport::Http { bind, bearer, .. } => {
                assert_eq!(bind.to_string(), "0.0.0.0:9000");
                assert_eq!(bearer.as_deref(), Some("secret"));
            }
            _ => panic!("expected http"),
        }
    }

    #[test]
    fn non_loopback_without_bearer_is_flagged() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[(VAR_HTTP_BIND, "0.0.0.0:8000")])).unwrap();
        assert!(cfg.transport.is_unauthenticated_non_loopback());
    }

    #[test]
    fn unknown_transport_names_accepted_values() {
        let tmp = TempDir::new().unwrap();
        let err = build(with_root(&tmp, &[(VAR_TRANSPORT, "carrier-pigeon")])).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("stdio") && msg.contains("http"));
    }

    #[test]
    fn custom_log_filter_applied() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[(VAR_LOG, "debug,agentmem=trace")])).unwrap();
        assert_eq!(cfg.log_filter, "debug,agentmem=trace");
    }

    #[test]
    fn cli_overrides_env() {
        // CLI http_bind beats the env value via as_overrides composition.
        let tmp = TempDir::new().unwrap();
        let cli = Cli {
            root_dir: Some(tmp.path().to_path_buf()),
            http_bind: Some("0.0.0.0:9000".to_string()),
            ..Default::default()
        };
        let overrides = cli.as_overrides();
        let env: HashMap<String, String> =
            [(VAR_HTTP_BIND.to_string(), "127.0.0.1:1111".to_string())]
                .into_iter()
                .collect();
        let cfg =
            Config::build(&|k| overrides.get(k).cloned().or_else(|| env.get(k).cloned())).unwrap();
        match cfg.transport {
            Transport::Http { bind, .. } => assert_eq!(bind.to_string(), "0.0.0.0:9000"),
            _ => panic!("expected http"),
        }
    }

    #[test]
    fn http_allowed_hosts_default_empty() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[])).unwrap();
        match cfg.transport {
            Transport::Http { allowed_hosts, .. } => assert!(allowed_hosts.is_empty()),
            _ => panic!("expected http"),
        }
    }

    #[test]
    fn http_allowed_hosts_parsed_and_trimmed() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(
            &tmp,
            &[(
                VAR_HTTP_ALLOWED_HOSTS,
                " agentmem.svc.cluster.local , agentmem.example.com:8000 , ",
            )],
        ))
        .unwrap();
        match cfg.transport {
            Transport::Http { allowed_hosts, .. } => assert_eq!(
                allowed_hosts,
                vec!["agentmem.svc.cluster.local", "agentmem.example.com:8000"]
            ),
            _ => panic!("expected http"),
        }
    }

    #[test]
    fn http_allowed_hosts_wildcard_retained() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[(VAR_HTTP_ALLOWED_HOSTS, "*")])).unwrap();
        match cfg.transport {
            Transport::Http { allowed_hosts, .. } => assert_eq!(allowed_hosts, vec!["*"]),
            _ => panic!("expected http"),
        }
    }

    #[test]
    fn stdio_ignores_http_allowed_hosts() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(
            &tmp,
            &[
                (VAR_TRANSPORT, "stdio"),
                (VAR_HTTP_ALLOWED_HOSTS, "example.com"),
            ],
        ))
        .unwrap();
        assert_eq!(cfg.transport, Transport::Stdio);
    }

    #[test]
    fn cli_overrides_env_for_http_allowed_hosts() {
        let tmp = TempDir::new().unwrap();
        let cli = Cli {
            root_dir: Some(tmp.path().to_path_buf()),
            http_allowed_hosts: Some("from-cli.example.com".to_string()),
            ..Default::default()
        };
        let overrides = cli.as_overrides();
        let env: HashMap<String, String> = [(
            VAR_HTTP_ALLOWED_HOSTS.to_string(),
            "from-env.example.com".to_string(),
        )]
        .into_iter()
        .collect();
        let cfg =
            Config::build(&|k| overrides.get(k).cloned().or_else(|| env.get(k).cloned())).unwrap();
        match cfg.transport {
            Transport::Http { allowed_hosts, .. } => {
                assert_eq!(allowed_hosts, vec!["from-cli.example.com"])
            }
            _ => panic!("expected http"),
        }
    }

    #[test]
    fn include_hidden_globs_default_empty() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[])).unwrap();
        assert!(cfg.include_hidden_globs.is_empty());
    }

    #[test]
    fn include_hidden_globs_parsed_and_trimmed() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(
            &tmp,
            &[(VAR_INCLUDE_HIDDEN_GLOBS, " .obsidian/** , **/.config , ")],
        ))
        .unwrap();
        assert_eq!(cfg.include_hidden_globs, vec![".obsidian/**", "**/.config"]);
    }

    #[test]
    fn include_hidden_globs_invalid_pattern_fails_fast() {
        let tmp = TempDir::new().unwrap();
        // An unclosed alternate group is an invalid glob.
        let err = build(with_root(&tmp, &[(VAR_INCLUDE_HIDDEN_GLOBS, "{a,b")])).unwrap_err();
        assert!(err.to_string().contains(VAR_INCLUDE_HIDDEN_GLOBS));
    }

    #[test]
    fn cli_overrides_env_for_include_hidden_globs() {
        let tmp = TempDir::new().unwrap();
        let cli = Cli {
            root_dir: Some(tmp.path().to_path_buf()),
            include_hidden_globs: Some(".obsidian/**".to_string()),
            ..Default::default()
        };
        let overrides = cli.as_overrides();
        let env: HashMap<String, String> = [(
            VAR_INCLUDE_HIDDEN_GLOBS.to_string(),
            ".cache/**".to_string(),
        )]
        .into_iter()
        .collect();
        let cfg =
            Config::build(&|k| overrides.get(k).cloned().or_else(|| env.get(k).cloned())).unwrap();
        assert_eq!(cfg.include_hidden_globs, vec![".obsidian/**"]);
    }
}
