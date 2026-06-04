//! Environment-variable-driven configuration with optional CLI overrides.
//!
//! The canonical configuration surface is the environment; CLI flags (parsed by
//! [`Cli`]) override the matching variable. Every variable except
//! `AGENTMEM_ROOT_DIR` has a default, and invalid values fail fast with a
//! human-readable message naming the offending variable.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use camino::Utf8PathBuf;
use chrono_tz::Tz;

use crate::error::AgentmemError;
use crate::policy::Policy;
use crate::template::Template;

/// The default agents-folder name.
pub const DEFAULT_AGENTS_DIR: &str = "Agents";
/// The default VFS suffix template.
pub const DEFAULT_TEMPLATE: &str = "<agent>.<user>";
/// The default HTTP bind address.
pub const DEFAULT_HTTP_BIND: &str = "127.0.0.1:8000";
/// The default tracing filter directive.
pub const DEFAULT_LOG_FILTER: &str = "warn,agentmem=info";

const VAR_ROOT_DIR: &str = "AGENTMEM_ROOT_DIR";
const VAR_AGENTS_DIR: &str = "AGENTMEM_AGENTS_DIR";
const VAR_TEMPLATE: &str = "AGENTMEM_VFS_TEMPLATE";
const VAR_POLICY: &str = "AGENTMEM_POLICY";
const VAR_TRANSPORT: &str = "AGENTMEM_TRANSPORT";
const VAR_HTTP_BIND: &str = "AGENTMEM_HTTP_BIND";
const VAR_HTTP_BEARER: &str = "AGENTMEM_HTTP_BEARER";
const VAR_TIMEZONE: &str = "AGENTMEM_TIMEZONE";
const VAR_HONOR_IGNORE: &str = "AGENTMEM_HONOR_IGNORE_FILES";
const VAR_INCLUDE_HIDDEN: &str = "AGENTMEM_INCLUDE_HIDDEN";
const VAR_LOG: &str = "AGENTMEM_LOG";

/// The selected transport and its parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Stdio,
    Http {
        bind: SocketAddr,
        bearer: Option<String>,
    },
}

impl Transport {
    /// `true` when bound on a non-loopback interface without a bearer token —
    /// the condition that warrants a startup warning.
    pub fn is_unauthenticated_non_loopback(&self) -> bool {
        match self {
            Transport::Http { bind, bearer } => bearer.is_none() && !bind.ip().is_loopback(),
            Transport::Stdio => false,
        }
    }
}

/// The fully-resolved server configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Canonical absolute vault root.
    pub root_dir: PathBuf,
    /// Agents folder relative to the root; empty means "the agents folder is the
    /// vault root".
    pub agents_dir: Utf8PathBuf,
    pub template: Template,
    pub policy: Policy,
    pub transport: Transport,
    pub timezone: Tz,
    pub honor_ignore_files: bool,
    pub include_hidden: bool,
    /// The `tracing_subscriber::EnvFilter` directive string.
    pub log_filter: String,
}

/// CLI flags that mirror — and override — the environment variables.
#[derive(Debug, Default, clap::Parser)]
#[command(name = "agentmem", version, about = "MCP server for multi-tenant agent memory")]
pub struct Cli {
    /// Vault root directory (overrides AGENTMEM_ROOT_DIR).
    #[arg(long)]
    pub root_dir: Option<PathBuf>,
    /// Agents folder name (overrides AGENTMEM_AGENTS_DIR).
    #[arg(long)]
    pub agents_dir: Option<String>,
    /// VFS suffix template (overrides AGENTMEM_VFS_TEMPLATE).
    #[arg(long)]
    pub vfs_template: Option<String>,
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
    /// IANA timezone (overrides AGENTMEM_TIMEZONE).
    #[arg(long)]
    pub timezone: Option<String>,
    /// Honour .gitignore/.obsidianignore (overrides AGENTMEM_HONOR_IGNORE_FILES).
    #[arg(long)]
    pub honor_ignore_files: Option<bool>,
    /// Include hidden dotfiles (overrides AGENTMEM_INCLUDE_HIDDEN).
    #[arg(long)]
    pub include_hidden: Option<bool>,
    /// Tracing filter directive (overrides AGENTMEM_LOG).
    #[arg(long)]
    pub log: Option<String>,
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
        if let Some(v) = &self.vfs_template {
            m.insert(VAR_TEMPLATE, v.clone());
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
        if let Some(v) = &self.timezone {
            m.insert(VAR_TIMEZONE, v.clone());
        }
        if let Some(v) = &self.honor_ignore_files {
            m.insert(VAR_HONOR_IGNORE, v.to_string());
        }
        if let Some(v) = &self.include_hidden {
            m.insert(VAR_INCLUDE_HIDDEN, v.to_string());
        }
        if let Some(v) = &self.log {
            m.insert(VAR_LOG, v.clone());
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
        let root_raw = get(VAR_ROOT_DIR).filter(|s| !s.is_empty()).ok_or_else(|| {
            config_err(format!("{VAR_ROOT_DIR} is required but was not set"))
        })?;
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

        // --- template ---
        let template_raw = get(VAR_TEMPLATE).unwrap_or_else(|| DEFAULT_TEMPLATE.to_string());
        let template = Template::parse(&template_raw).map_err(|e| {
            config_err(format!("{VAR_TEMPLATE} is invalid ({template_raw:?}): {e}"))
        })?;

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

        // --- logging ---
        let log_filter = get(VAR_LOG)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_string());

        Ok(Config {
            root_dir,
            agents_dir,
            template,
            policy,
            transport,
            timezone,
            honor_ignore_files,
            include_hidden,
            log_filter,
        })
    }

    /// A [`crate::path::PathResolver`] for this configuration.
    pub fn resolver(&self) -> crate::path::PathResolver {
        crate::path::PathResolver::new(
            self.root_dir.clone(),
            self.agents_dir.clone(),
            self.template.clone(),
        )
    }

    /// A human-readable multi-line summary for `--print-config`.
    pub fn describe(&self) -> String {
        let transport = match &self.transport {
            Transport::Stdio => "stdio".to_string(),
            Transport::Http { bind, bearer } => {
                format!("http bind={bind} bearer={}", if bearer.is_some() { "set" } else { "unset" })
            }
        };
        format!(
            "root_dir = {root}\n\
             agents_dir = {agents}\n\
             template = {template:?}\n\
             policy = {policy:?}\n\
             transport = {transport}\n\
             timezone = {tz}\n\
             honor_ignore_files = {ignore}\n\
             include_hidden = {hidden}\n\
             log_filter = {log}",
            root = self.root_dir.display(),
            agents = if self.agents_dir.as_str().is_empty() {
                "<vault root>"
            } else {
                self.agents_dir.as_str()
            },
            template = self.template,
            policy = self.policy,
            transport = transport,
            tz = self.timezone,
            ignore = self.honor_ignore_files,
            hidden = self.include_hidden,
            log = self.log_filter,
        )
    }

    #[cfg(test)]
    fn from_pairs(pairs: &[(&str, &str)]) -> Result<Config, AgentmemError> {
        let map: HashMap<String, String> =
            pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
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
            Ok(Transport::Http { bind, bearer })
        }
        other => Err(config_err(format!(
            "{VAR_TRANSPORT} must be one of [\"stdio\", \"http\"], got {other:?}"
        ))),
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
        let err =
            Config::from_pairs(&[(VAR_ROOT_DIR, file.to_str().unwrap())]).unwrap_err();
        assert!(err.to_string().contains("not a directory"));
    }

    #[test]
    fn all_defaults_apply() {
        let tmp = TempDir::new().unwrap();
        let cfg = build(with_root(&tmp, &[])).unwrap();
        assert_eq!(cfg.agents_dir.as_str(), "Agents");
        assert_eq!(cfg.template, Template::parse("<agent>.<user>").unwrap());
        assert_eq!(cfg.policy, Policy::Namespaced);
        assert_eq!(
            cfg.transport,
            Transport::Http {
                bind: default_http_bind(),
                bearer: None
            }
        );
        assert_eq!(cfg.timezone, Tz::UTC);
        assert!(cfg.honor_ignore_files);
        assert!(!cfg.include_hidden);
        assert_eq!(cfg.log_filter, DEFAULT_LOG_FILTER);
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
    fn malformed_template_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let err = build(with_root(&tmp, &[(VAR_TEMPLATE, "<agent")])).unwrap_err();
        assert!(err.to_string().contains(VAR_TEMPLATE));
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
            Transport::Http { bind, bearer } => {
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
        let cfg = Config::build(&|k| {
            overrides.get(k).cloned().or_else(|| env.get(k).cloned())
        })
        .unwrap();
        match cfg.transport {
            Transport::Http { bind, .. } => assert_eq!(bind.to_string(), "0.0.0.0:9000"),
            _ => panic!("expected http"),
        }
    }
}
