## Context

Visibility is enforced in two places that must agree:

- `Storage::walk_files` (`src/storage.rs:279`) drives **listings**. It uses the `ignore` crate's `WalkBuilder` with `.ignore(self.honor_ignore_files)` already enabled, plus a custom `.obsidianignore` filename, and applies hidden filtering by hand (`is_hidden`).
- `Storage::is_visible` (`src/storage.rs:190`) guards **direct** read/write/edit/delete. It calls `is_hidden` and `is_ignored`. `is_ignored` (`src/storage.rs:335`) assembles a `GitignoreBuilder` from only `.gitignore` and `.obsidianignore` — it omits the generic `.ignore` file that the walker already honours.

Result: a file matched only by `.ignore` is absent from listings but still directly addressable — the two sets disagree. Separately, hidden filtering is a single global boolean (`include_hidden`, `src/config.rs:79`); there is no way to keep dotfiles hidden by default yet expose one config directory.

Config is loaded via `Config::build` (`src/config.rs:200`) from env vars (prefix `AGENTMEM_`) with CLI overrides (`Cli`, `src/config.rs:91`). The `ignore` crate is already a dependency. This is a single-crate project.

## Goals / Non-Goals

**Goals:**
- Make `.ignore` a consulted source in `is_ignored`, so the addressable set matches the listed set for all three ignore-file kinds.
- Add a glob-based exception to hidden filtering: configured patterns un-hide matching dot-paths and their whole subtree.
- Preserve all current defaults exactly (no behavioural change when the new var is unset).

**Non-Goals:**
- Changing how the walker composes per-directory ignore files (already correct).
- Per-scope or per-tool visibility overrides.
- Removing or renaming `AGENTMEM_INCLUDE_HIDDEN` (it stays as the global switch).
- A general-purpose "force-include" that overrides ignore-file rules — ignore rules still apply to exempted dot-paths.

## Decisions

### D1 — Add `.ignore` to `is_ignored` assembly
In `is_ignored`, extend the `add_for` closure (`src/storage.rs:342`) to also `b.add(d.join(".ignore"))`, alongside `.gitignore` and `.obsidianignore`. This mirrors the walker, which already enables `.ignore`. No precedence subtleties: `GitignoreBuilder` treats added files as ordered rule sources; matching is "any source ignores it". Keep file order consistent (`.ignore`, `.gitignore`, `.obsidianignore`) but order is immaterial to the boolean `is_ignore()` outcome here.

**Nested composition is already handled and is preserved.** `is_ignored` walks `dir` from the vault root down through each component of `rel.parent()` (`src/storage.rs:340-354`), calling `add_for` at every level — so a nested `.gitignore`/`.obsidianignore`/`.ignore` in any subfolder is added and composes with ancestor files, exactly like `git`. Because `.ignore` is added in the same closure, it inherits this per-directory behaviour for free; the walker already composes nested files via `WalkBuilder`. No new traversal logic is needed — only the test coverage in §1/§4 to lock the behaviour in.

*Alternative considered:* register `.ignore` as another `add_custom_ignore_filename` in the walker and rely on the walker alone. Rejected — `is_visible` is the direct-access gate and must independently reject; the walker doesn't run on a direct read.

### D2 — Compile include-globs into a single `Gitignore` matcher with parent-match semantics
Reuse the `ignore` crate: build a `GitignoreBuilder` rooted at the vault root, `add_line(None, pattern)` for each configured glob, and `build()` once at `Storage` construction. Store the compiled `Gitignore` (e.g. `include_hidden: Gitignore`) on `Storage`/derived from `Config`.

In `is_hidden`, after determining a path would be hidden, check the include matcher: a path is exempt when `matcher.matched_path_or_any_parents(abs, is_dir).is_ignore()` is true for the path **or any parent** — which is exactly the "match a directory ⇒ whole subtree exempt" semantics the user chose. Because we reuse gitignore matching, a pattern like `.obsidian` matches the directory and `matched_path_or_any_parents` propagates the match to descendants; `.obsidian/**` works equally. This keeps glob semantics identical to the rest of the system (ripgrep/Obsidian-compatible).

*Alternatives considered:* (a) `globset::GlobSet` — would require translating "match parent ⇒ include subtree" by hand and adds a second glob dialect; rejected for consistency. (b) plain name/prefix matching — rejected, the user chose glob patterns.

### D3 — Configuration surface
- New constant `VAR_INCLUDE_HIDDEN_GLOBS = "AGENTMEM_INCLUDE_HIDDEN_GLOBS"` (`src/config.rs`).
- New `Cli` field `--include-hidden-globs` (a `String`, comma-separated, to mirror env shape) following the existing override pattern.
- Parse comma-separated entries, trim whitespace, drop empties. Validate by attempting to compile the `Gitignore` at build time; on error exit non-zero with the offending pattern (consistent with the existing fail-fast style for invalid booleans/timezone).
- Add a `Config` field holding the patterns (`Vec<String>`) and/or the compiled matcher. Compile in `Storage` construction so `Config` stays cheaply `Clone`. Include the value in `--print-config` output.

*Alternative for separator:* newline-separated. Rejected — comma matches typical single-line env-var ergonomics; globs do not contain commas in practice.

### D4 — Interaction ordering in `is_visible` / `walk_files`
Hidden filtering becomes: `would be hidden by dot-rule` AND NOT `include-glob exempt`. The agents-folder exemption stays a separate, earlier short-circuit (unchanged). When `include_hidden=true`, `is_hidden` already returns "not hidden" for everything, so the glob matcher is never consulted — a natural no-op. Apply the identical check in both `is_visible` (`src/storage.rs:199`) and the in-loop filter of `walk_files` (`src/storage.rs:310`) so listings and direct access stay in lockstep.

## Risks / Trade-offs

- **[Listing/direct-access drift re-introduced]** → The two call sites (`walk_files` line 310, `is_visible` line 199) must use the same `is_hidden`/`is_ignored` logic. Mitigation: keep the exemption inside `is_hidden` itself so both sites inherit it; add a test asserting a `.ignore`-matched and an include-glob path behave identically under list vs. direct read.
- **[Over-broad glob exposes secrets]** → A careless pattern like `**` or `.*` could expose `.git`, `.ssh`-like material. Mitigation: empty default; document the feature as opt-in and scoped; ignore-file rules still apply on top. Not enforced in code (operator's responsibility), but called out in config docs.
- **[Per-call matcher rebuild cost]** → `is_ignored` already rebuilds a `GitignoreBuilder` per call; adding `.ignore` is negligible. The include-glob matcher is compiled once at startup, so `is_hidden` only does a match per path. No new per-call build for the glob path.
- **[Invalid glob discovered only at startup]** → Fail-fast at `Config::build`/`Storage` construction with a clear message; acceptable and consistent with existing validation.

## Migration Plan

Pure addition; no data migration. Deploy normally. Rollback is reverting the binary — defaults reproduce prior behaviour, and unset `AGENTMEM_INCLUDE_HIDDEN_GLOBS` means no functional change. Existing vaults with a `.ignore` file will see those paths become *consistently* hidden (they were already absent from listings); if an operator was relying on direct-access to a `.ignore`-matched file, they set `AGENTMEM_HONOR_IGNORE_FILES=false` or remove the `.ignore` rule.

## Open Questions

- None blocking. (Separator confirmed comma; matching confirmed glob; recursion confirmed whole-subtree.)
