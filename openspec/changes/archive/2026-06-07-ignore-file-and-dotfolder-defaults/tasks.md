## 1. Generic `.ignore` consistency (direct-access path)

- [x] 1.1 In `src/storage.rs` `is_ignored`, extend the `add_for` closure to also `b.add(d.join(".ignore"))` alongside `.gitignore` and `.obsidianignore`.
- [x] 1.2 Add a unit test asserting a path matched only by a `.ignore` rule is excluded from `list_visible` AND rejected by `is_visible` (so listing and direct access agree).
- [x] 1.3 Add a test asserting `AGENTMEM_HONOR_IGNORE_FILES=false` disables `.ignore` enforcement too.
- [x] 1.4 Add a test for nested per-directory composition: a `.gitignore`/`.ignore`/`.obsidianignore` in a subfolder excludes matching files within that subtree (listing and direct access), while files outside the subtree are unaffected — confirming `is_ignored` and the walker agree.

## 2. Configuration: include-hidden glob list

- [x] 2.1 Add `const VAR_INCLUDE_HIDDEN_GLOBS: &str = "AGENTMEM_INCLUDE_HIDDEN_GLOBS";` in `src/config.rs`.
- [x] 2.2 Add `--include-hidden-globs` field to `Cli` (comma-separated `String`) mirroring the existing override pattern and doc comment.
- [x] 2.3 Add an `include_hidden_globs: Vec<String>` field to `Config`; default empty.
- [x] 2.4 In `Config::build`, resolve the value (CLI overrides env), split on commas, trim, drop empty entries.
- [x] 2.5 Validate patterns by compiling a `Gitignore` (rooted at the vault root) at build time; on failure exit non-zero with a stderr message naming `AGENTMEM_INCLUDE_HIDDEN_GLOBS` and the offending pattern.
- [x] 2.6 Include the resolved glob list in the `--print-config` output.
- [x] 2.7 Add config tests: parsing/trimming, CLI-overrides-env precedence, empty default, and invalid-pattern fail-fast.

## 3. Storage: glob-based hidden exemption

- [x] 3.1 Compile the include-hidden glob list into a single `ignore::gitignore::Gitignore` matcher when constructing `Storage` (from `Config`); store it on `Storage`.
- [x] 3.2 In `is_hidden`, after the dot-segment determination, return not-hidden when the matcher's `matched_path_or_any_parents(abs, is_dir)` is an ignore-match for the path or any parent (whole-subtree exemption). Keep the agents-folder short-circuit unchanged and ahead of this check.
- [x] 3.3 Confirm both call sites (`is_visible` and the in-loop filter of `walk_files`) inherit the exemption via `is_hidden` (no logic duplicated at the call sites).

## 4. Storage tests for exemption semantics

- [x] 4.1 Test: `AGENTMEM_INCLUDE_HIDDEN_GLOBS=.obsidian/**` makes `.obsidian/app.json` and `.obsidian/plugins/x/data.json` visible and directly accessible, while a sibling `.git/config` / `.cache/tmp.md` stays hidden and returns `path_not_permitted`.
- [x] 4.2 Test: empty glob list reproduces current behaviour (all dot-segments excluded).
- [x] 4.3 Test: `AGENTMEM_INCLUDE_HIDDEN=true` makes the glob list a no-op (everything visible).
- [x] 4.4 Test: exempted dot-paths are still subject to ignore-file rules unless `AGENTMEM_HONOR_IGNORE_FILES=false`.
- [x] 4.5 Test: agents-folder exemption (`.agents` agents dir) is unaffected by an unrelated/empty glob list.

## 5. Documentation and verification

- [x] 5.1 Update README / configuration reference (env-var table) to document `AGENTMEM_INCLUDE_HIDDEN_GLOBS`, `--include-hidden-globs`, and that `.ignore` is honoured alongside `.gitignore`/`.obsidianignore`.
- [x] 5.2 Run `cargo fmt`, `cargo clippy`, and `cargo test` locally; ensure all pass before commit.
