//! The on-disk storage layer: atomic writes, search/replace edits, deletes,
//! directory walking with visibility filters, listing, and opaque pagination.
//!
//! All full-file writes use the temp-file + fsync + rename pattern for crash
//! safety. Concurrent writes to the same target within this process are
//! serialised by a per-target advisory lock. Cross-process races are tolerated
//! (last-writer-wins via the atomic rename), per design decision D5.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use camino::{Utf8Component, Utf8Path};
use dashmap::DashMap;
use ignore::WalkBuilder;

use crate::error::AgentmemError;
use crate::path::{PathResolver, PhysicalPath, VirtualPath};
use crate::policy::Region;

/// An opaque pagination cursor — a base64-encoded byte offset into the
/// deterministic listing order.
pub struct Cursor;

impl Cursor {
    pub fn encode(offset: u64) -> String {
        BASE64.encode(offset.to_string().as_bytes())
    }

    pub fn decode(cursor: &str) -> Result<u64, AgentmemError> {
        let bytes = BASE64.decode(cursor).map_err(|_| AgentmemError::InvalidArgument {
            message: "cursor is not valid".to_string(),
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|_| AgentmemError::InvalidArgument {
            message: "cursor is not valid".to_string(),
        })?;
        text.parse::<u64>().map_err(|_| AgentmemError::InvalidArgument {
            message: "cursor is not valid".to_string(),
        })
    }
}

/// The on-disk storage layer.
pub struct Storage {
    resolver: PathResolver,
    honor_ignore_files: bool,
    include_hidden: bool,
    locks: DashMap<PathBuf, Arc<Mutex<()>>>,
}

impl Storage {
    pub fn new(resolver: PathResolver, honor_ignore_files: bool, include_hidden: bool) -> Storage {
        Storage {
            resolver,
            honor_ignore_files,
            include_hidden,
            locks: DashMap::new(),
        }
    }

    pub fn resolver(&self) -> &PathResolver {
        &self.resolver
    }

    /// Read a file's UTF-8 contents.
    pub fn read(&self, physical: &PhysicalPath) -> Result<String, AgentmemError> {
        match std::fs::read(physical.as_path()) {
            Ok(bytes) => String::from_utf8(bytes).map_err(|_| AgentmemError::Io {
                kind: std::io::ErrorKind::InvalidData,
                context: "reading note (not valid UTF-8)",
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AgentmemError::NotFound {
                virtual_path: self.display_path(physical),
            }),
            Err(e) => Err(AgentmemError::io("reading note", &e)),
        }
    }

    /// Atomically write the full contents of a file: temp file in the same
    /// directory, fsync, then rename over the target. Returns the byte count.
    pub fn write_atomic(
        &self,
        physical: &PhysicalPath,
        content: &str,
    ) -> Result<usize, AgentmemError> {
        self.with_target_lock(physical.as_path(), || self.write_atomic_locked(physical, content))
    }

    /// Read the current contents (treating a missing file as absent), hand them
    /// to `f` to compute the new contents, and persist atomically — all under the
    /// per-target lock so concurrent callers serialise. Used by the diary append.
    pub fn read_modify_write(
        &self,
        physical: &PhysicalPath,
        f: impl FnOnce(Option<String>) -> String,
    ) -> Result<usize, AgentmemError> {
        self.with_target_lock(physical.as_path(), || {
            let current = match std::fs::read_to_string(physical.as_path()) {
                Ok(s) => Some(s),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(AgentmemError::io("reading note", &e)),
            };
            let next = f(current);
            self.write_atomic_locked(physical, &next)
        })
    }

    fn write_atomic_locked(
        &self,
        physical: &PhysicalPath,
        content: &str,
    ) -> Result<usize, AgentmemError> {
        self.mkdirs_for(physical)?;
        let parent = physical.as_path().parent().ok_or(AgentmemError::Io {
            kind: std::io::ErrorKind::InvalidInput,
            context: "resolving parent directory",
        })?;

        let mut temp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| AgentmemError::io("creating temp file", &e))?;
        temp.write_all(content.as_bytes())
            .map_err(|e| AgentmemError::io("writing temp file", &e))?;
        temp.as_file()
            .sync_all()
            .map_err(|e| AgentmemError::io("syncing temp file", &e))?;
        temp.persist(physical.as_path())
            .map_err(|e| AgentmemError::io("renaming temp file", &e.error))?;
        Ok(content.len())
    }

    /// Replace the unique occurrence of `search` with `replace`, persisting
    /// atomically. The search string must occur exactly once.
    pub fn edit_search_replace(
        &self,
        physical: &PhysicalPath,
        search: &str,
        replace: &str,
    ) -> Result<usize, AgentmemError> {
        self.with_target_lock(physical.as_path(), || {
            let current = self.read(physical)?;
            let count = current.matches(search).count();
            match count {
                0 => Err(AgentmemError::EditSearchNotFound),
                1 => {
                    let updated = current.replacen(search, replace, 1);
                    self.write_atomic_locked(physical, &updated)?;
                    Ok(search.len())
                }
                n => Err(AgentmemError::EditSearchAmbiguous { count: n }),
            }
        })
    }

    /// Delete a single file. Never removes directories; leaves an emptied parent
    /// in place.
    pub fn delete(&self, physical: &PhysicalPath) -> Result<(), AgentmemError> {
        self.with_target_lock(physical.as_path(), || {
            match std::fs::remove_file(physical.as_path()) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AgentmemError::NotFound {
                    virtual_path: self.display_path(physical),
                }),
                Err(e) => Err(AgentmemError::io("deleting note", &e)),
            }
        })
    }

    /// Create any missing parent directories for a write target.
    pub fn mkdirs_for(&self, physical: &PhysicalPath) -> Result<(), AgentmemError> {
        if let Some(parent) = physical.as_path().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AgentmemError::io("creating parent directories", &e))?;
        }
        Ok(())
    }

    /// Whether a resolved physical path is visible (not hidden, not ignored)
    /// under the active filters. Direct read/write/edit/delete call this to
    /// reject excluded paths with `path_not_permitted` before any IO.
    pub fn is_visible(&self, physical: &PhysicalPath) -> bool {
        let root = self.resolver.vault_root();
        let Ok(rel_std) = physical.as_path().strip_prefix(root) else {
            return false;
        };
        let Some(rel) = Utf8Path::from_path(rel_std) else {
            return false;
        };

        if !self.include_hidden && self.is_hidden(rel) {
            return false;
        }
        if self.honor_ignore_files && self.is_ignored(rel) {
            return false;
        }
        true
    }

    /// List the caller's own-scope files inside the agents folder as clean
    /// virtual paths.
    pub fn list_inside_agents_folder(
        &self,
        rendered_scope: &str,
    ) -> Result<Vec<VirtualPath>, AgentmemError> {
        let agents_root = self.agents_root();
        let mut out = Vec::new();
        for (physical, _rel) in self.walk_files(&agents_root) {
            if let Some(clean) = self.resolver.strip_suffix(&physical, rendered_scope) {
                out.push(clean);
            }
        }
        Ok(out)
    }

    /// List files outside the agents folder (but inside the vault root) as clean
    /// virtual paths.
    pub fn list_outside_agents_folder(&self) -> Result<Vec<VirtualPath>, AgentmemError> {
        if self.resolver.agents_dir().as_str().is_empty() {
            // The agents folder is the vault root; there is no outside region.
            return Ok(Vec::new());
        }
        let root = self.resolver.vault_root().to_path_buf();
        let agents_dir = self.resolver.agents_dir().to_owned();
        let mut out = Vec::new();
        for (_physical, rel) in self.walk_files(&root) {
            if rel.starts_with(&agents_dir) {
                continue;
            }
            out.push(VirtualPath::new(rel.as_str())?);
        }
        Ok(out)
    }

    /// All virtual paths visible to a scope across the supplied regions,
    /// deduplicated and deterministically ordered.
    pub fn list_visible(
        &self,
        rendered_scope: &str,
        regions: &[Region],
    ) -> Result<Vec<VirtualPath>, AgentmemError> {
        let mut set: BTreeSet<String> = BTreeSet::new();
        for region in regions {
            let paths = match region {
                Region::InsideAgentsFolder => self.list_inside_agents_folder(rendered_scope)?,
                Region::OutsideAgentsFolder => self.list_outside_agents_folder()?,
            };
            for p in paths {
                set.insert(p.as_str().to_string());
            }
        }
        set.into_iter()
            .map(|s| VirtualPath::new(&s))
            .collect::<Result<Vec<_>, _>>()
    }

    // --- internals ---

    fn agents_root(&self) -> PathBuf {
        let dir = self.resolver.agents_dir();
        if dir.as_str().is_empty() {
            self.resolver.vault_root().to_path_buf()
        } else {
            self.resolver.vault_root().join(dir.as_str())
        }
    }

    /// Walk a start directory, applying ignore-file filtering (via the `ignore`
    /// crate) and hidden filtering (by hand, so the agents folder is exempt even
    /// when it begins with `.`). Returns `(physical, rel_to_root)` for each file.
    fn walk_files(&self, start: &Path) -> Vec<(PathBuf, camino::Utf8PathBuf)> {
        if !start.exists() {
            return Vec::new();
        }
        let root = self.resolver.vault_root().to_path_buf();
        let mut builder = WalkBuilder::new(start);
        builder
            .hidden(false) // hidden filtering done by hand (agents-folder exemption)
            .parents(self.honor_ignore_files)
            .git_global(self.honor_ignore_files)
            .git_ignore(self.honor_ignore_files)
            .git_exclude(self.honor_ignore_files)
            .ignore(self.honor_ignore_files)
            .require_git(false)
            .follow_links(false);
        if self.honor_ignore_files {
            builder.add_custom_ignore_filename(".obsidianignore");
        }

        let mut out = Vec::new();
        for entry in builder.build().flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let Ok(rel_std) = path.strip_prefix(&root) else {
                continue;
            };
            let Some(rel) = Utf8Path::from_path(rel_std) else {
                continue;
            };
            if !self.include_hidden && self.is_hidden(rel) {
                continue;
            }
            out.push((path.to_path_buf(), rel.to_owned()));
        }
        out
    }

    /// A path is hidden if any segment begins with `.`, except segments that are
    /// part of the agents-folder prefix (so a `.agents` folder stays visible).
    fn is_hidden(&self, rel: &Utf8Path) -> bool {
        let agents = self.resolver.agents_dir();
        let to_check: &Utf8Path = if !agents.as_str().is_empty() && rel.starts_with(agents) {
            rel.strip_prefix(agents).unwrap_or(rel)
        } else {
            rel
        };
        to_check
            .components()
            .any(|c| matches!(c, Utf8Component::Normal(s) if s.starts_with('.')))
    }

    /// Whether `rel` (relative to the vault root) is matched by a `.gitignore` or
    /// `.obsidianignore` rule, assembled from the vault root down to the file's
    /// parent directory.
    fn is_ignored(&self, rel: &Utf8Path) -> bool {
        use ignore::gitignore::GitignoreBuilder;
        let root = self.resolver.vault_root();
        let mut builder = GitignoreBuilder::new(root);

        // Collect ignore files from root down to the file's parent directory.
        let mut dir = root.to_path_buf();
        let add_for = |b: &mut GitignoreBuilder, d: &Path| {
            b.add(d.join(".gitignore"));
            b.add(d.join(".obsidianignore"));
        };
        add_for(&mut builder, &dir);
        if let Some(parent) = rel.parent() {
            for comp in parent.components() {
                if let Utf8Component::Normal(seg) = comp {
                    dir.push(seg);
                    add_for(&mut builder, &dir);
                }
            }
        }

        let Ok(gitignore) = builder.build() else {
            return false;
        };
        let abs = root.join(rel.as_str());
        gitignore
            .matched_path_or_any_parents(&abs, false)
            .is_ignore()
    }

    /// Run `f` while holding the per-target advisory lock. The backing `Mutex`
    /// lives in the `DashMap` entry; we clone its `Arc` so the guard's lifetime is
    /// sound regardless of map churn.
    fn with_target_lock<R>(&self, target: &Path, f: impl FnOnce() -> R) -> R {
        let arc = self
            .locks
            .entry(target.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _guard = arc.lock().unwrap_or_else(|p| p.into_inner());
        f()
    }

    fn display_path(&self, physical: &PhysicalPath) -> String {
        physical
            .as_path()
            .strip_prefix(self.resolver.vault_root())
            .ok()
            .and_then(Utf8Path::from_path)
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| "<note>".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::Template;
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use camino::Utf8PathBuf;

    fn storage(tmp: &TempDir, agents: &str, template: &str, honor: bool, hidden: bool) -> Storage {
        let resolver = PathResolver::new(
            tmp.path().canonicalize().unwrap(),
            Utf8PathBuf::from(agents),
            Template::parse(template).unwrap(),
        );
        Storage::new(resolver, honor, hidden)
    }

    fn vp(s: &str) -> VirtualPath {
        VirtualPath::new(s).unwrap()
    }

    #[test]
    fn cursor_round_trips() {
        for offset in [0u64, 1, 50, 12345, u64::MAX] {
            assert_eq!(Cursor::decode(&Cursor::encode(offset)).unwrap(), offset);
        }
        assert!(Cursor::decode("!!!not base64!!!").is_err());
    }

    #[test]
    fn write_then_read_round_trips() {
        let tmp = TempDir::new().unwrap();
        let s = storage(&tmp, "Agents", "<agent>.<user>", true, false);
        let physical = s.resolver.resolve("coder.alice", &vp("Agents/PERSONA.md")).unwrap();
        let n = s.write_atomic(&physical, "hello").unwrap();
        assert_eq!(n, 5);
        assert_eq!(s.read(&physical).unwrap(), "hello");
    }

    #[test]
    fn read_missing_is_not_found() {
        let tmp = TempDir::new().unwrap();
        let s = storage(&tmp, "Agents", "<agent>.<user>", true, false);
        let physical = s.resolver.resolve("coder.alice", &vp("Agents/missing.md")).unwrap();
        assert!(matches!(s.read(&physical), Err(AgentmemError::NotFound { .. })));
    }

    #[test]
    fn write_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let s = storage(&tmp, "Agents", "<agent>.<user>", true, false);
        let physical = s
            .resolver
            .resolve("coder.alice", &vp("Agents/deep/nested/note.md"))
            .unwrap();
        s.write_atomic(&physical, "x").unwrap();
        assert!(physical.as_path().exists());
    }

    #[test]
    fn edit_unique_succeeds_missing_and_ambiguous_fail() {
        let tmp = TempDir::new().unwrap();
        let s = storage(&tmp, "Agents", "<agent>.<user>", true, false);
        let physical = s.resolver.resolve("coder.alice", &vp("Agents/n.md")).unwrap();

        s.write_atomic(&physical, "alpha beta gamma").unwrap();
        s.edit_search_replace(&physical, "beta", "BETA").unwrap();
        assert_eq!(s.read(&physical).unwrap(), "alpha BETA gamma");

        assert!(matches!(
            s.edit_search_replace(&physical, "zeta", "z"),
            Err(AgentmemError::EditSearchNotFound)
        ));

        s.write_atomic(&physical, "dup dup").unwrap();
        assert!(matches!(
            s.edit_search_replace(&physical, "dup", "x"),
            Err(AgentmemError::EditSearchAmbiguous { count: 2 })
        ));
    }

    #[test]
    fn delete_removes_file_and_missing_is_not_found() {
        let tmp = TempDir::new().unwrap();
        let s = storage(&tmp, "Agents", "<agent>.<user>", true, false);
        let physical = s.resolver.resolve("coder.alice", &vp("Agents/d.md")).unwrap();
        s.write_atomic(&physical, "x").unwrap();
        s.delete(&physical).unwrap();
        assert!(!physical.as_path().exists());
        assert!(matches!(s.delete(&physical), Err(AgentmemError::NotFound { .. })));
    }

    #[test]
    fn listing_shows_only_own_scope() {
        let tmp = TempDir::new().unwrap();
        let s = storage(&tmp, "Agents", "<agent>.<user>", true, false);
        for (scope, name) in [
            ("coder.alice", "Agents/notes.md"),
            ("coder.bob", "Agents/notes.md"),
            ("writer.alice", "Agents/notes.md"),
        ] {
            let p = s.resolver.resolve(scope, &vp(name)).unwrap();
            s.write_atomic(&p, "x").unwrap();
        }
        let listed = s.list_inside_agents_folder("coder.alice").unwrap();
        let strs: Vec<_> = listed.iter().map(|p| p.as_str().to_string()).collect();
        assert_eq!(strs, vec!["Agents/notes.md"]);
    }

    #[test]
    fn hidden_files_excluded_but_agents_dot_dir_visible() {
        let tmp = TempDir::new().unwrap();
        // agents folder begins with '.': must stay visible.
        let s = storage(&tmp, ".agents", "", true, false);
        let visible = s.resolver.resolve("", &vp(".agents/notes.md")).unwrap();
        let hidden = s.resolver.resolve("", &vp(".agents/.tmp.md")).unwrap();
        s.write_atomic(&visible, "x").unwrap();
        s.write_atomic(&hidden, "x").unwrap();

        let listed = s.list_inside_agents_folder("").unwrap();
        let strs: Vec<_> = listed.iter().map(|p| p.as_str().to_string()).collect();
        assert!(strs.contains(&".agents/notes.md".to_string()));
        assert!(!strs.iter().any(|s| s.contains(".tmp.md")));

        assert!(s.is_visible(&visible));
        assert!(!s.is_visible(&hidden));
    }

    #[test]
    fn gitignore_excludes_matched_files() {
        let tmp = TempDir::new().unwrap();
        tmp.child(".gitignore").write_str("*.wip.md\n").unwrap();
        let s = storage(&tmp, "Agents", "", true, false);

        let kept = s.resolver.resolve("", &vp("Agents/keep.md")).unwrap();
        let ignored = s.resolver.resolve("", &vp("Agents/draft.wip.md")).unwrap();
        s.write_atomic(&kept, "x").unwrap();
        s.write_atomic(&ignored, "x").unwrap();

        let listed = s.list_inside_agents_folder("").unwrap();
        let strs: Vec<_> = listed.iter().map(|p| p.as_str().to_string()).collect();
        assert!(strs.contains(&"Agents/keep.md".to_string()));
        assert!(!strs.iter().any(|s| s.contains("draft.wip.md")));
        assert!(!s.is_visible(&ignored));
        assert!(s.is_visible(&kept));
    }

    /// Concurrent writes to the same diary file are serialised by the per-target
    /// lock so no append is lost or interleaved.
    #[test]
    fn concurrent_appends_are_serialised() {
        let tmp = TempDir::new().unwrap();
        let s = Arc::new(storage(&tmp, "Agents", "<agent>.<user>", true, false));
        let physical = Arc::new(
            s.resolver.resolve("coder.alice", &vp("Agents/diary/d.md")).unwrap(),
        );

        let mut handles = Vec::new();
        for i in 0..16 {
            let s = s.clone();
            let physical = physical.clone();
            handles.push(std::thread::spawn(move || {
                s.read_modify_write(&physical, |cur| {
                    format!("{}line{i}\n", cur.unwrap_or_default())
                })
                .unwrap();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let final_contents = s.read(&physical).unwrap();
        assert_eq!(final_contents.lines().count(), 16);
    }
}
