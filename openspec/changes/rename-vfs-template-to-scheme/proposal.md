## Why

The word "template" is currently bound to the VFS path-shaping concept (`AGENTMEM_VFS_TEMPLATE`, the dotted `<agent>.<user>` string), but the most intuitive meaning of "template" is a document with fill-in slots — exactly what the planned session-context document is. Reserving "template" for the path concept forced the session-context work to adopt the defensive name "layout". Renaming the path concept frees "template" for its natural use and gives the path concept a more accurate name: it is a structured addressing **scheme**, not a fill-in document.

## What Changes

- **BREAKING**: Rename the environment variable `AGENTMEM_VFS_TEMPLATE` → `AGENTMEM_VFS_SCHEME`. No backward-compatible alias is provided (pre-release; no public consumers).
- **BREAKING**: Rename the CLI override flag `--vfs-template` → `--vfs-scheme`.
- Rename the in-code concept from "VFS template" to "VFS scheme" throughout: the `Template` type → `Scheme`, `src/template.rs` → `src/scheme.rs`, `TemplateError`/`RenderError` and their messages, and every doc comment and identifier that says "template" in the path-shaping sense.
- Rename the spec-level requirement and scenario language from "VFS (suffix) template" to "VFS scheme" across the four affected capabilities.
- This is a **pure terminology rename with no behavior change**: the grammar (`segment := placeholder | literal`), the rendering rules, the scope-key contract, path resolution, and the empty-scheme/suffix-disabling semantics all stay exactly as they are. Defaults are unchanged (`<agent>.<user>`).

Out of scope: introducing the session-context "template" itself — that remains the separately-proposed `configurable-session-context` change, which will be revised to build on this new vocabulary.

## Capabilities

### New Capabilities
<!-- None — this is a rename of existing capabilities. -->

### Modified Capabilities
- `configuration`: the `AGENTMEM_VFS_TEMPLATE` variable is renamed to `AGENTMEM_VFS_SCHEME`; the "VFS suffix template" requirement and its scenarios are restated in terms of "VFS scheme"; the env-vars-read list and the defaults scenario are updated.
- `vault-storage`: the "VFS template resolution" requirement and its scenarios are restated as "VFS scheme resolution"; behavior is identical.
- `mcp-server`: the `tools/list` scenarios that reference the configured VFS template and the invalid-config startup scenario are restated in terms of `AGENTMEM_VFS_SCHEME`.
- `memory-tools`: the scope-parameter contract requirement that references `AGENTMEM_VFS_TEMPLATE` is restated in terms of `AGENTMEM_VFS_SCHEME`.

## Impact

- **Code**: `src/template.rs` (→ `src/scheme.rs`, `Template` → `Scheme`), `src/config.rs` (env var constant, `Cli` flag, `Config.template` field, `describe()`, tests), `src/path.rs` (`PathResolver` field/accessor and the many `template` references), `src/tools.rs` (`template()` accessor, schema merge, doc comments), `src/storage.rs`, `src/policy.rs`, `src/mcp.rs`, `src/lib.rs` (module export).
- **Tests/snapshots**: `tests/schema_snapshots.rs` and `tests/tools.rs` identifier updates; the four `tests/snapshots/schema_snapshots__*.snap` files regenerate (their JSON *content* is unchanged — only field descriptions reference scope keys, not the word "template" — so snapshots should match after the code rename, but they are reviewed to confirm).
- **Docs**: `README.md` env-var table and any prose mentioning the VFS template.
- **Compatibility**: BREAKING for any environment setting `AGENTMEM_VFS_TEMPLATE` or passing `--vfs-template`; those must switch to `AGENTMEM_VFS_SCHEME` / `--vfs-scheme`. No runtime behavior, default, or on-disk layout changes.
- **Downstream**: unblocks the `configurable-session-context` change to use "template" for the session-context document.
