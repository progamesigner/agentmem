## Context

`AGENTMEM_VFS_TEMPLATE` and its `Template` type (`src/template.rs`) shape VFS paths: a dotted string of literal and `<ident>` segments that defines the required scope keys and renders to a per-scope directory segment plus a filename suffix. The concept is shipped (archived `build-agentmem-mcp-server`, synced into the four main specs). Separately, the planned `configurable-session-context` change introduces an operator-authored document with `{{…}}` fill-in slots — the thing most people would call a "template" — and was forced to adopt the defensive name "layout" purely to avoid colliding with the path concept's claim on "template".

This change reclaims the word by renaming the path concept to **scheme**, a more accurate term for a structured addressing convention. It is a vocabulary change only; no runtime behavior, default, grammar, or on-disk layout changes.

## Goals / Non-Goals

**Goals:**
- Rename `AGENTMEM_VFS_TEMPLATE` → `AGENTMEM_VFS_SCHEME` and `--vfs-template` → `--vfs-scheme`.
- Rename the in-code concept (`Template` → `Scheme`, `src/template.rs` → `src/scheme.rs`, error types, fields, accessors, doc comments) so the codebase reads consistently.
- Restate the four affected specs in terms of "scheme".
- Preserve 100% of existing behavior; existing tests pass on meaning, snapshots match after the rename.

**Non-Goals:**
- Introducing the session-context "template" — that is the separate `configurable-session-context` change (revised to depend on this one).
- Any backward-compatible alias for the old variable/flag (hard rename; pre-release).
- Changing the scheme grammar, rendering, defaults, scope contract, or path-resolution semantics.

## Decisions

### D1: "scheme" as the replacement word
"Scheme" connotes a structured naming/addressing convention (cf. URI scheme) and does not collide with any existing identifier in the codebase (unlike "schema", which is pervasive via `schemars`/JSON Schema). Chosen over "pattern" (which implies matching/globbing rather than generating) and "layout" (which would reuse the very word being freed from the session-context proposal).

### D2: Keep the `AGENTMEM_VFS_` prefix
The variable stays `AGENTMEM_VFS_SCHEME` rather than re-anchoring on `AGENTMEM_SCOPE_*`. The `VFS_` prefix leads with the path-shaping role and sits naturally beside `AGENTMEM_AGENTS_DIR`; re-anchoring would be a larger conceptual churn than this rename intends.

### D3: Hard rename, no alias
`AGENTMEM_VFS_TEMPLATE` is removed outright; setting it has no effect, and the server starts with the default scheme. No deprecation shim is read. Justified by pre-release status (`0.1.0`, no public consumers).

### D4: Rename the module and type, not just the env var
`src/template.rs` → `src/scheme.rs`; `Template` → `Scheme`; `TemplateError` → `SchemeError`; `Config.template` → `Config.scheme`; `PathResolver::template()` → `PathResolver::scheme()`; `Toolbox::template()` → `Toolbox::scheme()`. Leaving the internal type as `Template` while the env var said "scheme" would reintroduce exactly the naming confusion this change removes. `RenderError` keeps its name (it is about rendering, not specific to the word "template").

### D5: Spec deltas use RENAMED + MODIFIED for the two renamed requirements
Two requirement *headers* contain the word: `configuration` → "VFS suffix template" and `vault-storage` → "VFS template resolution". Each is expressed as a `RENAMED Requirements` (FROM/TO) entry to change the header plus a `MODIFIED Requirements` block (under the new name) carrying the full reworded body. The `mcp-server` and `memory-tools` requirements keep their names and are plain `MODIFIED` (scenario/description wording only).

## Risks / Trade-offs

- **Snapshot drift** (the four `schema_snapshots__*.snap`) → these snapshots capture tool input-schema JSON, whose scope-field descriptions reference scope *key names* (`agent`, `user`), not the word "template". Expectation: snapshots match unchanged after the rename. Mitigation: run the snapshot tests; if any differ, review with `cargo insta` and confirm the diff is purely incidental before accepting.
- **Missed reference** (a lingering "template" in the path sense) → Mitigation: after the sweep, `grep -rni "template" src/ tests/ README.md` and confirm every remaining hit refers to JSON *schema* generation or is otherwise unrelated, not the VFS concept.
- **RENAMED + MODIFIED on one requirement may not validate** as expected by the OpenSpec tooling → Mitigation: run `openspec validate rename-vfs-template-to-scheme --strict` and adjust the delta representation if the validator rejects the pairing.
- **Downstream coupling** with `configurable-session-context` → that change's artifacts still say "layout"; they will be revised after this lands. Until then the two proposals describe overlapping vocabulary. Mitigation: sequence the work — land this rename first.

## Migration Plan

1. Land this change; operators update any `AGENTMEM_VFS_TEMPLATE` / `--vfs-template` usage to `AGENTMEM_VFS_SCHEME` / `--vfs-scheme`. No on-disk migration — vault contents are untouched.
2. Revise `configurable-session-context` to use "template" for the session-context document and drop "layout".

## Open Questions

- None blocking. The session-context env-var naming (`AGENTMEM_SESSION_CONTEXT_FILE` vs `…_TEMPLATE`) is deferred to the `configurable-session-context` change.
