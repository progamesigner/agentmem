# Security & trust model

This document describes what `agentmem` guarantees, what it deliberately does not
guarantee in v1, and how to widen or narrow its visibility.

## Threat model at a glance

| Concern | v1 stance |
|---|---|
| Claimed scope keys (`agent`, `user`, …) | **Trusted.** Any client reachable through the configured transport may address any scope. Per-tenant authentication is deferred. |
| Path traversal (`..`, absolute paths, symlink escape) | **Prevented.** Structurally impossible to escape the vault root. |
| Cross-scope access inside the agents folder | **Structurally impossible.** The resolver always appends the caller's own scope. |
| Writes into human-owned regions | **Governed by policy.** Denied unless the active policy permits it. |
| HTTP endpoint exposure | **Optional static bearer token.** Loopback-only by default. |

## Claimed scope is trusted (v1)

Scope keys are supplied **per tool call**, not bound to an authenticated identity.
A single server process can therefore serve many agents concurrently with no
session handshake — but it also means a misbehaving or malicious client could
present another agent's scope keys and read or write that scope's files.

This is an intentional v1 simplification. The architecture keeps the boundary
clean so that adding authentication later is purely additive: a middleware that
validates the claimed scope keys against an authenticated identity (OAuth claims,
mTLS subject, or per-tenant tokens) and rejects mismatches at the tool boundary.

The HTTP transport's `AGENTMEM_HTTP_BEARER` protects the **endpoint**, not
individual tenants — it is a coarse gate, not per-scope authorization.

> **Operational guidance:** until per-tenant auth lands, only expose `agentmem` to
> clients you trust to honestly declare their own scope. For shared or remote
> deployments, set `AGENTMEM_HTTP_BEARER` and bind a non-loopback interface only
> behind a trusted network boundary.

## Traversal is not trusted

Every client-supplied virtual path is validated and normalised:

- empty paths, embedded NUL bytes, absolute paths, and any `..` component are
  rejected before resolution;
- after resolution, the deepest existing ancestor of the target is canonicalised
  and must lie within the canonical vault root — this catches `..` survivors and
  **symlink escapes** (a symlink inside the vault pointing outside is refused);
- anything resolving outside the vault root is always denied, regardless of policy.

## Own-scope strictness is structural

Inside the agents folder, with a non-empty template, the resolver appends the
caller's rendered scope as both the first directory segment and the file-stem
suffix on **every** read, write, edit, and delete. A request for `PERSONA.md` from
`{agent: coder, user: alice}` can only ever resolve to
`Agents/coder.alice/PERSONA.coder.alice.md`. There is no virtual path — legitimate
or crafted — that addresses another scope's file: a crafted path such as
`Agents/PERSONA.coder.bob.md` still resolves under `coder.alice` and lands at
`Agents/coder.alice/PERSONA.coder.bob.coder.alice.md`, never bob's file. Listings
likewise filter by the caller's own suffix, so other scopes' files are invisible.

An empty template (`AGENTMEM_VFS_TEMPLATE=`) disables suffixing: the agents folder
degenerates into a plain shared directory governed by the policy's outside-folder
rules, with no own-scope isolation.

## The four policies and their guarantees outside the agents folder

There are exactly two regions: **inside** the agents folder and **outside** it but
still within the vault root. One server-wide policy (`AGENTMEM_POLICY`) governs
both:

| Policy | Inside agents folder | Outside agents folder |
|---|---|---|
| `scoped` | own-scope read/write | **denied** (`path_not_permitted`) |
| `namespaced` *(default)* | own-scope read/write | read-only (writes → `write_denied`) |
| `readonly` | own-scope read-only | read-only (writes → `write_denied`) |
| `readwrite` | own-scope read/write | read/write |

The distinction between the two refusal codes is deliberate: a region that is
entirely unreachable (`scoped` outside) reports `path_not_permitted` and does not
confirm whether a file exists; a region that is readable but not writable reports
`write_denied`. When the agents folder is the vault root (`AGENTMEM_AGENTS_DIR=.`),
the "outside" region is empty and that column has no effect.

## Visibility filters

To avoid surfacing editor/VCS noise and to keep agents from trampling tool state,
two filters apply to listing **and** to direct read/write/edit/delete:

- **Hidden filter** — any path segment beginning with `.` is excluded by default.
  The configured agents folder is exempt even if it begins with `.` (e.g.
  `AGENTMEM_AGENTS_DIR=.agents` stays traversable). Toggle off with
  `AGENTMEM_INCLUDE_HIDDEN=true`.
- **Ignore-file filter** — `.gitignore` and `.obsidianignore` patterns are honoured
  hierarchically (the same machinery `ripgrep` uses). Toggle off with
  `AGENTMEM_HONOR_IGNORE_FILES=false`.

Direct addressing of an excluded path returns `path_not_permitted` — **not**
`not_found` — so the filter does not leak whether the file actually exists.

### Widening visibility

Set `AGENTMEM_INCLUDE_HIDDEN=true` and/or `AGENTMEM_HONOR_IGNORE_FILES=false` when
an operator genuinely wants the agent to see hidden or ignored files. Both default
to the conservative setting.

## Error hygiene

Internal errors are mapped at the MCP boundary into a human-readable message plus
a structured `code`. Messages reference the **virtual** path the client supplied,
never the resolved physical path, and raw OS error strings are never propagated —
IO failures carry only an error kind and a static context label.

## Deferred to follow-up changes

- **Per-tenant authentication** binding scope keys to authenticated identities.
- **CORS / auth presets** for non-loopback HTTP deployments.
