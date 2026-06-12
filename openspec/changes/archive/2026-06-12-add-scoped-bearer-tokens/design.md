## Context

Today the HTTP transport has one optional static bearer enforced by an axum
middleware (`src/transport/http.rs:195`); past that gate, scope keys supplied
as tool arguments are trusted verbatim (`docs/security.md`, "scope keys are
trusted, not authenticated"). Scope extraction is already centralized: every
tool funnels through `Toolbox::scope_map` (`src/tools.rs:302`), and the
resource/prompt/`/v1/context` surfaces funnel through
`render_session_context`'s validation (`src/tools.rs:374`). That
centralization is what makes per-tenant authorization a contained change: the
grant check slots into the two functions every scoped request already passes
through.

## Goals / Non-Goals

**Goals:**
- A client's bearer determines which scopes it may name; everything else about
  the tool surface is unchanged.
- Deny-before-IO: an unauthorized scope never reaches path resolution or
  storage.
- Full coverage of every scoped surface: tools, resource, prompt,
  `/v1/context`.

**Non-Goals:**
- Stdio authentication (the launching process owns the vault; unchanged by
  design).
- Hot reload / rotation without restart (operators restart the sidecar; noted
  as possible follow-up).
- OAuth/OIDC or upstream-proxy auth integration (a reverse proxy can still
  terminate fancier auth in front; this change is the server-native floor).
- Per-policy or per-path grants (grants bind scopes, not regions; the
  server-wide policy still governs regions).

## Decisions

- **Grant file, not an env-encoded grant list.** Multi-token × multi-key
  grants do not fit a flat env var legibly, and files are the natural shape
  for Kubernetes secret mounts. Format:
  `{ "tokens": [ { "token": "…", "scopes": { "<placeholder>": "<exact>"|"*" } } ] }`.
  Startup validation fails fast: every `scopes` key must be a scheme
  placeholder, every placeholder must be present in each entry, duplicate
  tokens union their grants. An unreadable or invalid file is a startup error,
  not a silent open server.
- **The static bearer composes as the all-scopes grant.** `AGENTMEM_HTTP_BEARER`
  keeps its exact semantics (operator/admin token); setting only the tokens
  file means every caller is scope-bound. Neither set keeps today's
  unauthenticated behavior (with the existing startup WARN).
- **Authn in middleware, authz at scope validation.** The middleware resolves
  the presented bearer to a `Grant` (`AllScopes` or a set of per-key
  matchers) and rejects unknown bearers with 401. The grant — not the token —
  travels with the request; the token string is dropped immediately and never
  logged. Authorization happens where scope maps are already validated
  (`scope_map` / `render_session_context` / the `/v1/context` handler):
  requested keys are matched against the grant, mismatch → `scope_denied`
  (tool-result domain error; 403 JSON on `/v1/context`). Checking at scope
  validation rather than in the middleware means the transport never parses
  JSON-RPC bodies and the rule covers every present and future scoped surface
  by construction.
- **Grant transport: HTTP request parts propagated into the rmcp request
  context.** rmcp's streamable HTTP service exposes the originating request's
  parts/extensions to handlers; the middleware inserts the resolved `Grant`
  as a request extension, and the server reads it (absent extension on the
  stdio path = `AllScopes`). An early spike task pins the exact rmcp API;
  the documented fallback is constructing the per-session server through the
  service factory from a session-scoped grant (the factory closure already
  clones the server per session) — same model, different plumbing.
- **`scope_denied` is a distinct error code.** Agents must distinguish "this
  scope is not yours" (re-authenticate / fix configuration) from
  `invalid_argument` (malformed call) and `path_not_permitted` (policy). The
  message names the offending key but never enumerates valid grants.
- **Wildcards are per-key and total only.** `"user": "*"` matches any value;
  partial patterns (`"user": "t*"`) are rejected at startup. Exact-or-star
  keeps grants auditable and avoids glob-injection questions in a security
  feature.

## Risks / Trade-offs

- [rmcp extension-propagation API may differ across versions] → Spike task
  first; fallback (per-session factory binding) is designed up front, and the
  authz seam (`scope_map`) is identical either way.
- [Tokens in a file on disk] → Standard secret-mount practice; README
  documents permissions expectations; tokens are never logged (the config
  `Debug` impl redacts, as it must for the static bearer today — verify in
  tests).
- [No hot reload: rotation requires restart] → Acceptable for a sidecar;
  documented. Follow-up if demanded.
- [SSE session opened before a grant change] → Sessions resolve their grant
  per request at scope validation, so a revoked token fails on the next call
  even on a live session (grants are not cached beyond the request).

## Migration Plan

Purely additive: deployments without `AGENTMEM_HTTP_TOKENS_FILE` behave
byte-identically (including the unauthenticated WARN). Rollback = unset the
variable. `docs/security.md`'s trust-model section moves per-tenant auth from
"deferred" to "delivered", and the container README gains a tokens-file
example.
