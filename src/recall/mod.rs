//! In-memory content recall.
//!
//! Recall is backed by per-scope in-memory indexes plus a single shared-region
//! index — nothing is written to disk. A scope's notes live only in that scope's
//! index, so a query opens only the caller's own-scope index and (policy
//! permitting) the shared index: cross-scope recall is *structurally* impossible,
//! not filtered. Indexes are built eagerly at startup, updated synchronously on the
//! server's own writes, and reconciled against external edits by a stat-diff that a
//! filesystem watcher (and a freshness window) trigger.
//!
//! The backend is configurable. The default [`SimpleIndex`] supports
//! case-insensitive substring and regex matching; the opt-in tantivy backend (the
//! `recall-tantivy` feature) adds BM25 ranking and frontmatter property filters.
//! Scores from the scope and shared indexes are normalized to 0–1 per index before
//! merging, so the agent-facing score is comparable across the two corpora.

mod simple;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime};

use crate::config::{RecallBackendKind, RecallConfig};
use crate::error::AgentmemError;
use crate::path::PhysicalPath;
use crate::policy::Region;
use crate::storage::{Cursor, Storage};

/// The maximum number of snippets returned per hit.
pub(crate) const MAX_SNIPPETS: usize = 3;
/// The maximum length, in bytes, of a single snippet before truncation.
pub(crate) const MAX_SNIPPET_LEN: usize = 200;

// --- public request / response types ---

/// A frontmatter property predicate (honoured by the tantivy backend only).
#[derive(Debug, Clone)]
pub struct PropertyFilter {
    pub key: String,
    pub op: FilterOp,
    pub value: Option<String>,
}

/// The comparison a [`PropertyFilter`] applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Exists,
    Eq,
    Contains,
    Gt,
    Lt,
    Ge,
    Le,
}

/// A parsed recall request (after scope extraction and argument validation).
#[derive(Debug, Clone)]
pub struct RecallQuery {
    /// Full-text query (substring on `simple`, BM25 on tantivy).
    pub text: Option<String>,
    /// Regular-expression query over note content.
    pub regex: Option<String>,
    /// Frontmatter property predicates (tantivy backend only).
    pub filters: Vec<PropertyFilter>,
    /// Clean-path prefix relative to the agents folder, mirroring `list_memory_notes`.
    pub path_prefix: Option<String>,
    /// Page size (already defaulted/capped by the caller).
    pub limit: u64,
    /// Pagination offset, decoded from an opaque cursor.
    pub offset: u64,
}

/// One ranked recall hit returned to the agent.
#[derive(Debug, Clone)]
pub struct RecallHit {
    pub path: String,
    pub score: f32,
    pub snippets: Vec<String>,
}

/// A page of recall hits.
#[derive(Debug, Clone)]
pub struct RecallResults {
    pub hits: Vec<RecallHit>,
    pub next_cursor: Option<String>,
    /// `true` when a regex scan hit the byte cap and stopped early.
    pub truncated: bool,
}

// --- backend abstraction ---

/// A raw, un-normalized hit from a single backend index.
pub(crate) struct RawHit {
    pub clean_path: String,
    pub raw_score: f32,
    pub snippets: Vec<String>,
}

/// The outcome of scanning one index.
pub(crate) struct ScanResult {
    pub hits: Vec<RawHit>,
    pub truncated: bool,
}

/// A compiled query handed to a backend index.
pub(crate) struct CompiledQuery {
    /// Lower-cased substring needle, if a `text` query was supplied.
    pub substring: Option<String>,
    /// Compiled regex, if a `regex` query was supplied.
    pub regex: Option<regex::Regex>,
}

/// One in-memory backend index over a single region's notes.
pub(crate) trait BackendIndex: Send {
    fn upsert(&mut self, clean_path: &str, body: &str);
    fn remove(&mut self, clean_path: &str);
    fn query(&self, query: &CompiledQuery, byte_cap: usize) -> ScanResult;
}

// --- per-region index state ---

/// Per-file metadata used for stat-diff reconciliation.
struct FileMeta {
    clean_path: String,
    mtime: SystemTime,
    size: u64,
}

/// Which region an index covers.
#[derive(Clone)]
enum IndexRegion {
    /// A per-scope index inside the agents folder, keyed by its rendered scope.
    Scoped(String),
    /// The single shared-region index outside the agents folder.
    Shared,
}

/// One resident in-memory index plus its reconciliation bookkeeping.
struct RegionIndex {
    region: IndexRegion,
    manifest: BTreeMap<PathBuf, FileMeta>,
    backend: Box<dyn BackendIndex>,
    last_reconcile: Option<Instant>,
    last_access: Instant,
}

/// The mutable engine state behind a single lock.
struct EngineState {
    built: bool,
    shared: Option<RegionIndex>,
    scopes: HashMap<String, RegionIndex>,
}

/// The recall engine. Holds the in-memory indexes and serves queries; shared
/// behind the `Toolbox`'s `Arc`. Construction yields `None` when recall is `off`.
pub struct RecallEngine {
    effective: RecallBackendKind,
    storage: std::sync::Arc<Storage>,
    config: RecallConfig,
    state: Mutex<EngineState>,
    /// Set true once the eager build has completed; read by `GET /readyz`.
    ready: AtomicBool,
    /// Set by the filesystem watcher; forces the next query to reconcile.
    dirty: std::sync::Arc<AtomicBool>,
    /// The live watcher; kept alive for the engine's lifetime.
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

impl RecallEngine {
    /// Build an engine for the configured backend, or `None` when recall is `off`.
    /// Resolves the effective backend against the `recall-tantivy` feature, logging
    /// a fallback to `simple` when tantivy was requested but is unavailable.
    pub fn new(storage: std::sync::Arc<Storage>, config: RecallConfig) -> Option<RecallEngine> {
        let effective = match config.backend {
            RecallBackendKind::Off => return None,
            RecallBackendKind::Simple => RecallBackendKind::Simple,
            // The tantivy backend is wired in by the `recall-tantivy` feature
            // (added in a later change); until then a tantivy request falls back
            // to the simple backend.
            RecallBackendKind::Tantivy => {
                tracing::warn!(
                    "AGENTMEM_RECALL_BACKEND=tantivy but the tantivy backend is not built in; \
                     falling back to the simple backend"
                );
                RecallBackendKind::Simple
            }
        };
        Some(RecallEngine {
            effective,
            storage,
            config,
            state: Mutex::new(EngineState {
                built: false,
                shared: None,
                scopes: HashMap::new(),
            }),
            ready: AtomicBool::new(false),
            dirty: std::sync::Arc::new(AtomicBool::new(false)),
            watcher: Mutex::new(None),
        })
    }

    /// The effective backend after feature resolution.
    pub fn effective_backend(&self) -> RecallBackendKind {
        self.effective
    }

    /// Whether the effective backend can apply frontmatter property filters.
    pub fn supports_property_filters(&self) -> bool {
        matches!(self.effective, RecallBackendKind::Tantivy)
    }

    /// `true` once the eager startup build has completed. Backs `GET /readyz`.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    /// Eagerly build every scope index and the shared index, then mark ready. Safe
    /// to call repeatedly; the build runs once. This is also the block-until-ready
    /// path: a query arriving before the background build finishes takes the lock
    /// and builds inline.
    pub fn warm(&self) {
        let mut state = self.state.lock().expect("recall state poisoned");
        self.ensure_built(&mut state);
    }

    fn ensure_built(&self, state: &mut EngineState) {
        if state.built {
            return;
        }
        // Shared region (absent when the agents folder is the vault root).
        if !self.storage.resolver().agents_dir().as_str().is_empty() {
            let mut idx = self.new_region_index(IndexRegion::Shared);
            self.reconcile(&mut idx);
            state.shared = Some(idx);
        }
        // Every scope directory present on disk.
        let mut scope_dirs = self.storage.list_scope_dirs();
        // Single-tenant (empty scheme): one index under the empty rendered scope.
        if self.storage.resolver().scheme().is_empty() {
            scope_dirs.push(String::new());
        }
        for scope in scope_dirs {
            let mut idx = self.new_region_index(IndexRegion::Scoped(scope.clone()));
            self.reconcile(&mut idx);
            state.scopes.insert(scope, idx);
        }
        state.built = true;
        self.ready.store(true, Ordering::Release);
        tracing::info!(
            backend = self.effective.as_str(),
            scopes = state.scopes.len(),
            "recall index ready"
        );
    }

    /// Start the filesystem watcher: any change under the vault root marks the
    /// engine dirty, so the next query reconciles. Idempotent.
    pub fn start_watcher(&self) {
        use notify::{RecursiveMode, Watcher};
        let mut guard = self.watcher.lock().expect("recall watcher poisoned");
        if guard.is_some() {
            return;
        }
        let dirty = self.dirty.clone();
        let mut watcher = match notify::recommended_watcher(move |res: notify::Result<_>| {
            if res.is_ok() {
                dirty.store(true, Ordering::Release);
            }
        }) {
            Ok(w) => w,
            Err(err) => {
                tracing::warn!(%err, "recall filesystem watcher unavailable; relying on the freshness reconcile");
                return;
            }
        };
        let root = self.storage.resolver().vault_root().to_path_buf();
        if let Err(err) = watcher.watch(&root, RecursiveMode::Recursive) {
            tracing::warn!(%err, "recall watcher could not watch the vault root");
            return;
        }
        *guard = Some(watcher);
    }

    /// Incrementally update the index after the server's own write to `physical`.
    /// A no-op when the owning index is not currently resident (it will pick the
    /// change up on its next build).
    pub fn on_write(&self, rendered_scope: &str, region: Region, physical: &PhysicalPath) {
        let mut state = self.state.lock().expect("recall state poisoned");
        if !state.built {
            // Not built yet: the eager build will read the new content.
            return;
        }
        let idx = match region {
            Region::OutsideAgentsFolder => state.shared.as_mut(),
            Region::InsideAgentsFolder => state.scopes.get_mut(rendered_scope),
        };
        if let Some(idx) = idx {
            self.apply_path(idx, physical);
        }
    }

    /// Run a recall query for the caller's scope across the permitted regions.
    pub fn recall(
        &self,
        rendered_scope: &str,
        regions: &[Region],
        query: &RecallQuery,
    ) -> Result<RecallResults, AgentmemError> {
        if !query.filters.is_empty() && !self.supports_property_filters() {
            return Err(AgentmemError::Unsupported {
                message: "frontmatter property filters require the tantivy backend \
                          (build with --features recall-tantivy and set \
                          AGENTMEM_RECALL_BACKEND=tantivy)"
                    .to_string(),
            });
        }
        let compiled = compile_query(query)?;

        let mut state = self.state.lock().expect("recall state poisoned");
        self.ensure_built(&mut state);
        let force = self.dirty.swap(false, Ordering::AcqRel);

        let include_shared = regions.contains(&Region::OutsideAgentsFolder);
        let include_scope = regions.contains(&Region::InsideAgentsFolder);

        let mut merged: Vec<(f32, RawHit)> = Vec::new();
        let mut truncated = false;

        if include_scope {
            self.ensure_scope_resident(&mut state, rendered_scope);
            if let Some(idx) = state.scopes.get_mut(rendered_scope) {
                Self::refresh(idx, force, self.config.freshness, &self.storage);
                idx.last_access = Instant::now();
                let scan = idx
                    .backend
                    .query(&compiled, self.config.regex_scan_byte_cap);
                truncated |= scan.truncated;
                push_normalized(&mut merged, scan.hits);
            }
        }
        if include_shared && let Some(idx) = state.shared.as_mut() {
            Self::refresh(idx, force, self.config.freshness, &self.storage);
            let scan = idx
                .backend
                .query(&compiled, self.config.regex_scan_byte_cap);
            truncated |= scan.truncated;
            push_normalized(&mut merged, scan.hits);
        }
        drop(state);

        self.evict_if_needed();

        // Path-prefix filter, mirroring list_memory_notes (prefix relative to the
        // agents folder).
        if let Some(prefix) = &query.path_prefix {
            let agents = self.storage.resolver().agents_dir();
            let effective = if agents.as_str().is_empty() {
                prefix.clone()
            } else {
                format!("{agents}/{prefix}")
            };
            let with_sep = format!("{effective}/");
            merged
                .retain(|(_, h)| h.clean_path == effective || h.clean_path.starts_with(&with_sep));
        }

        // Sort by normalized score (desc), then path (asc) for stable ordering.
        merged.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.clean_path.cmp(&b.1.clean_path))
        });

        let total = merged.len() as u64;
        let start = query.offset.min(total) as usize;
        let end = (query.offset + query.limit).min(total) as usize;
        let next_cursor = if (end as u64) < total {
            Some(Cursor::encode(end as u64))
        } else {
            None
        };

        let hits = merged[start..end]
            .iter()
            .map(|(score, h)| RecallHit {
                path: h.clean_path.clone(),
                score: *score,
                // Strip the caller's own suffix from snippets, mirroring the read path.
                snippets: h
                    .snippets
                    .iter()
                    .map(|s| {
                        crate::wikilink::strip_links(s, rendered_scope, self.storage.resolver())
                    })
                    .collect(),
            })
            .collect();

        Ok(RecallResults {
            hits,
            next_cursor,
            truncated,
        })
    }

    // --- index construction / reconciliation ---

    fn new_region_index(&self, region: IndexRegion) -> RegionIndex {
        let backend: Box<dyn BackendIndex> = match self.effective {
            RecallBackendKind::Simple => Box::new(simple::SimpleIndex::default()),
            // `new` resolves Tantivy → Simple until the backend is built in.
            RecallBackendKind::Tantivy | RecallBackendKind::Off => {
                Box::new(simple::SimpleIndex::default())
            }
        };
        RegionIndex {
            region,
            manifest: BTreeMap::new(),
            backend,
            last_reconcile: None,
            last_access: Instant::now(),
        }
    }

    /// Reconcile if the index is stale or the engine was marked dirty.
    fn refresh(
        idx: &mut RegionIndex,
        force: bool,
        freshness: std::time::Duration,
        storage: &Storage,
    ) {
        let stale = match idx.last_reconcile {
            None => true,
            Some(t) => t.elapsed() >= freshness,
        };
        if force || stale {
            reconcile_with(idx, storage);
        }
    }

    fn reconcile(&self, idx: &mut RegionIndex) {
        reconcile_with(idx, &self.storage);
    }

    /// Re-read and upsert a single physical path into `idx` (or remove it if gone).
    fn apply_path(&self, idx: &mut RegionIndex, physical: &PhysicalPath) {
        apply_path_with(idx, physical, &self.storage);
    }

    /// Build a scope index on demand when it was never built or was evicted.
    fn ensure_scope_resident(&self, state: &mut EngineState, rendered_scope: &str) {
        if state.scopes.contains_key(rendered_scope) {
            return;
        }
        let mut idx = self.new_region_index(IndexRegion::Scoped(rendered_scope.to_string()));
        self.reconcile(&mut idx);
        state.scopes.insert(rendered_scope.to_string(), idx);
    }

    /// Evict the least-recently-accessed scope indexes beyond the resident cap.
    fn evict_if_needed(&self) {
        let cap = self.config.max_resident_scopes.max(1);
        let mut state = self.state.lock().expect("recall state poisoned");
        while state.scopes.len() > cap {
            let victim = state
                .scopes
                .iter()
                .min_by_key(|(_, idx)| idx.last_access)
                .map(|(k, _)| k.clone());
            match victim {
                Some(k) => {
                    state.scopes.remove(&k);
                }
                None => break,
            }
        }
    }
}

/// Gather the current visible files for an index's region as `(physical, clean)`.
fn current_files(idx: &RegionIndex, storage: &Storage) -> Vec<(PhysicalPath, String)> {
    let resolver = storage.resolver();
    let clean_paths = match &idx.region {
        IndexRegion::Shared => storage.list_outside_agents_folder().unwrap_or_default(),
        IndexRegion::Scoped(scope) => storage.list_inside_agents_folder(scope).unwrap_or_default(),
    };
    let scope_for_resolve = match &idx.region {
        IndexRegion::Shared => "",
        IndexRegion::Scoped(scope) => scope.as_str(),
    };
    let mut out = Vec::new();
    for vpath in clean_paths {
        if let Ok(physical) = resolver.resolve(scope_for_resolve, &vpath) {
            out.push((physical, vpath.as_str().to_string()));
        }
    }
    out
}

/// Stat-diff reconcile: upsert new/changed files, drop deleted ones.
fn reconcile_with(idx: &mut RegionIndex, storage: &Storage) {
    let current = current_files(idx, storage);
    let mut seen: BTreeMap<PathBuf, ()> = BTreeMap::new();

    for (physical, clean) in &current {
        let key = physical.as_path().to_path_buf();
        seen.insert(key.clone(), ());
        let meta = std::fs::metadata(physical.as_path());
        let (mtime, size) = match meta {
            Ok(m) => (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()),
            Err(_) => continue,
        };
        let unchanged = idx
            .manifest
            .get(&key)
            .is_some_and(|prev| prev.mtime == mtime && prev.size == size);
        if unchanged {
            continue;
        }
        if let Ok(body) = storage.read(physical) {
            idx.backend.upsert(clean, &body);
            idx.manifest.insert(
                key,
                FileMeta {
                    clean_path: clean.clone(),
                    mtime,
                    size,
                },
            );
        }
    }

    // Drop files that vanished.
    let removed: Vec<PathBuf> = idx
        .manifest
        .keys()
        .filter(|k| !seen.contains_key(*k))
        .cloned()
        .collect();
    for key in removed {
        if let Some(meta) = idx.manifest.remove(&key) {
            idx.backend.remove(&meta.clean_path);
        }
    }

    idx.last_reconcile = Some(Instant::now());
}

/// Upsert or remove a single physical path (used by the synchronous own-write path).
fn apply_path_with(idx: &mut RegionIndex, physical: &PhysicalPath, storage: &Storage) {
    let key = physical.as_path().to_path_buf();
    match std::fs::metadata(physical.as_path()) {
        Ok(m) => {
            let mtime = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let size = m.len();
            // Recompute the clean path for the manifest entry.
            let clean = idx
                .manifest
                .get(&key)
                .map(|meta| meta.clean_path.clone())
                .or_else(|| clean_path_for(idx, physical, storage));
            if let (Some(clean), Ok(body)) = (clean, storage.read(physical)) {
                idx.backend.upsert(&clean, &body);
                idx.manifest.insert(
                    key,
                    FileMeta {
                        clean_path: clean,
                        mtime,
                        size,
                    },
                );
            }
        }
        Err(_) => {
            if let Some(meta) = idx.manifest.remove(&key) {
                idx.backend.remove(&meta.clean_path);
            }
        }
    }
}

/// Derive the clean virtual path of a physical file for the given index region.
fn clean_path_for(idx: &RegionIndex, physical: &PhysicalPath, storage: &Storage) -> Option<String> {
    let resolver = storage.resolver();
    match &idx.region {
        IndexRegion::Scoped(scope) => resolver
            .strip_suffix(physical.as_path(), scope)
            .map(|v| v.as_str().to_string()),
        IndexRegion::Shared => {
            let rel = physical
                .as_path()
                .strip_prefix(resolver.vault_root())
                .ok()?;
            camino::Utf8Path::from_path(rel).map(|p| p.as_str().to_string())
        }
    }
}

/// Compile a query for the backends, validating the regex.
fn compile_query(query: &RecallQuery) -> Result<CompiledQuery, AgentmemError> {
    let substring = query
        .text
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase());
    let regex = match query.regex.as_ref().filter(|s| !s.is_empty()) {
        Some(pattern) => {
            Some(
                regex::Regex::new(pattern).map_err(|e| AgentmemError::InvalidArgument {
                    message: format!("invalid regex: {e}"),
                })?,
            )
        }
        None => None,
    };
    Ok(CompiledQuery { substring, regex })
}

/// Normalize one index's raw scores to 0–1 (by its own max) and append to `merged`.
fn push_normalized(merged: &mut Vec<(f32, RawHit)>, hits: Vec<RawHit>) {
    let max = hits.iter().map(|h| h.raw_score).fold(0.0_f32, f32::max);
    for hit in hits {
        let score = if max > 0.0 { hit.raw_score / max } else { 1.0 };
        merged.push((score, hit));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::PathResolver;
    use crate::scheme::Scheme;
    use std::sync::Arc;

    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    const BOTH: &[Region] = &[Region::InsideAgentsFolder, Region::OutsideAgentsFolder];

    /// Build an engine over a two-scope vault with a shared note.
    fn engine() -> (TempDir, RecallEngine) {
        let tmp = TempDir::new().unwrap();
        // coder.alice owns a note; coder.bob owns a note that also says "borrow".
        tmp.child("Agents/coder.alice/topics/rust.coder.alice.md")
            .write_str("The borrow checker enforces ownership.")
            .unwrap();
        tmp.child("Agents/coder.bob/topics/secret.coder.bob.md")
            .write_str("Bob's borrow secret lives here.")
            .unwrap();
        // A shared note outside the agents folder.
        tmp.child("Actions/release.md")
            .write_str("The release borrow process is documented.")
            .unwrap();

        let resolver = PathResolver::new(
            tmp.path().canonicalize().unwrap(),
            camino::Utf8PathBuf::from("Agents"),
            Scheme::parse("<agent>.<user>").unwrap(),
        );
        let storage = Arc::new(Storage::new(resolver, true, false, &[]));
        let config = RecallConfig {
            backend: RecallBackendKind::Simple,
            watch_debounce: std::time::Duration::from_millis(0),
            regex_scan_byte_cap: usize::MAX,
            max_resident_scopes: 256,
            freshness: std::time::Duration::from_millis(0),
        };
        let engine = RecallEngine::new(storage, config).unwrap();
        (tmp, engine)
    }

    fn query(text: &str) -> RecallQuery {
        RecallQuery {
            text: Some(text.to_string()),
            regex: None,
            filters: Vec::new(),
            path_prefix: None,
            limit: 100,
            offset: 0,
        }
    }

    #[test]
    fn warm_makes_engine_ready() {
        let (_tmp, engine) = engine();
        assert!(!engine.is_ready());
        engine.warm();
        assert!(engine.is_ready());
    }

    #[test]
    fn recall_finds_own_scope_and_shared_but_never_another_scope() {
        let (_tmp, engine) = engine();
        let results = engine
            .recall("coder.alice", BOTH, &query("borrow"))
            .unwrap();
        let paths: Vec<&str> = results.hits.iter().map(|h| h.path.as_str()).collect();

        assert!(paths.contains(&"Agents/topics/rust.md"));
        assert!(paths.contains(&"Actions/release.md"));
        // Structural isolation: bob's note is in a different index and unreachable.
        for hit in &results.hits {
            assert!(!hit.path.contains("bob"), "leaked path: {}", hit.path);
            assert!(!hit.path.contains("secret"), "leaked path: {}", hit.path);
            for snip in &hit.snippets {
                assert!(!snip.contains("Bob"), "leaked snippet: {snip}");
            }
        }
    }

    #[test]
    fn scoped_policy_omits_shared_region() {
        let (_tmp, engine) = engine();
        let results = engine
            .recall(
                "coder.alice",
                &[Region::InsideAgentsFolder],
                &query("borrow"),
            )
            .unwrap();
        let paths: Vec<&str> = results.hits.iter().map(|h| h.path.as_str()).collect();
        assert!(paths.contains(&"Agents/topics/rust.md"));
        assert!(!paths.contains(&"Actions/release.md"));
    }

    #[test]
    fn scores_are_normalized_zero_to_one() {
        let (_tmp, engine) = engine();
        let results = engine
            .recall("coder.alice", BOTH, &query("borrow"))
            .unwrap();
        assert!(!results.hits.is_empty());
        for hit in &results.hits {
            assert!(hit.score > 0.0 && hit.score <= 1.0, "score {}", hit.score);
        }
    }

    #[test]
    fn property_filters_are_unsupported_on_simple() {
        let (_tmp, engine) = engine();
        let mut q = query("borrow");
        q.filters.push(PropertyFilter {
            key: "tag".to_string(),
            op: FilterOp::Eq,
            value: Some("rust".to_string()),
        });
        let err = engine.recall("coder.alice", BOTH, &q).unwrap_err();
        assert_eq!(err.code(), crate::error::ErrorCode::Unsupported);
    }

    #[test]
    fn own_write_is_reflected_immediately() {
        let (tmp, engine) = engine();
        engine.warm();
        // A new own-scope note, then the synchronous own-write hook.
        let path = tmp.child("Agents/coder.alice/topics/async.coder.alice.md");
        path.write_str("Futures are polled by the executor.")
            .unwrap();
        let resolver = engine.storage.resolver();
        let vpath = crate::path::VirtualPath::new("Agents/topics/async.md").unwrap();
        let physical = resolver.resolve("coder.alice", &vpath).unwrap();
        engine.on_write("coder.alice", Region::InsideAgentsFolder, &physical);

        let results = engine
            .recall("coder.alice", BOTH, &query("futures"))
            .unwrap();
        let paths: Vec<&str> = results.hits.iter().map(|h| h.path.as_str()).collect();
        assert!(paths.contains(&"Agents/topics/async.md"));
    }
}
