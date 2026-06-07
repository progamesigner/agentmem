## Why

Two gaps weaken the vault's visibility model:

1. The directory walker already honours a generic `.ignore` file (via the `ignore` crate's `WalkBuilder.ignore(...)`), but the direct read/write/edit/delete path check only assembles `.gitignore` and `.obsidianignore`. A file matched solely by `.ignore` is hidden from listings yet still directly addressable — the visible set and the addressable set disagree.

2. Hidden filtering is all-or-nothing: `AGENTMEM_INCLUDE_HIDDEN` either excludes every dot-segment or none. There is no way to keep dotfiles excluded by default while selectively exposing a specific config directory (e.g. `.obsidian`) that an agent legitimately needs to manage.

## What Changes

- Treat a generic `.ignore` file as a first-class ignore source in the **direct path** visibility check (`is_ignored`), matching the walker's existing behaviour. After this change, `.ignore`, `.gitignore`, and `.obsidianignore` are all consulted consistently across listing and direct read/write/edit/delete.
- Honour **nested** ignore files (`.ignore`, `.gitignore`, `.obsidianignore`) per-directory, just like `git`: a file in any subfolder applies to that subtree and composes with files higher up. This is already how `is_ignored` assembles rules (root down to the target's parent) and how the walker behaves; the `.ignore` addition inherits the same per-directory composition. Made explicit here so it is covered by tests.
- Keep the default of **excluding every path with a dot-prefixed segment** (`AGENTMEM_INCLUDE_HIDDEN=false`), mirroring how unix-like systems hide dotfiles. The new glob list is the only way to opt specific dot-paths back in.
- Add a new configuration input — an **include-list of glob patterns** — that un-hides dot-paths matching any pattern, **including their whole subtree**, while all other dot-segments remain excluded by default. Exposed as env var `AGENTMEM_INCLUDE_HIDDEN_GLOBS` and a mirroring `--include-hidden-globs` CLI flag.
  - Patterns are gitignore-style globs evaluated relative to the vault root (e.g. `.obsidian/**`, `**/.config`).
  - A path is treated as not-hidden if it, or any of its parent directories, matches an include glob — so matching a directory un-hides everything beneath it.
  - The list is empty by default, preserving today's "exclude all dotfiles" behaviour.
- Interaction: when `AGENTMEM_INCLUDE_HIDDEN=true`, all dotfiles are already visible and the glob list is a no-op. Ignore-file rules still apply unless `AGENTMEM_HONOR_IGNORE_FILES=false`.

No breaking changes: defaults are unchanged (`INCLUDE_HIDDEN=false`, empty include-glob list, `HONOR_IGNORE_FILES=true`).

## Capabilities

### New Capabilities
<!-- None: this extends existing behaviour rather than introducing a new capability area. -->

### Modified Capabilities
- `vault-storage`: The "Visibility filters" requirement gains the generic `.ignore` file as a consulted ignore source on the direct-path check, and a glob-based exception to hidden-segment exclusion (with whole-subtree semantics).
- `configuration`: The "Visibility filter variables" requirement gains `AGENTMEM_INCLUDE_HIDDEN_GLOBS` (and its `--include-hidden-globs` CLI mirror), and documents that the generic `.ignore` file is honoured alongside `.gitignore`/`.obsidianignore` under `AGENTMEM_HONOR_IGNORE_FILES`.

## Impact

- **Code**: `src/storage.rs` (`is_ignored` adds `.ignore`; `is_hidden` consults the include-glob matcher with parent-match semantics), `src/config.rs` (new env var + CLI flag, parsing, `Config` field, `--print-config` output), `src/mcp.rs`/storage construction (thread the compiled glob set into `Storage`).
- **Specs**: delta updates to `vault-storage` and `configuration`.
- **Dependencies**: none new — reuses the already-present `ignore` crate (`Gitignore`/`GitignoreBuilder`) for glob matching with parent-match semantics.
- **Compatibility**: backward-compatible; unset/empty configuration reproduces current behaviour exactly.
