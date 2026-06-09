## Context

The HTTP transport (`src/transport/http.rs`) builds the `rmcp` Streamable HTTP service with `StreamableHttpServerConfig::default()`. In `rmcp` 1.7, that default sets `allowed_hosts = ["localhost", "127.0.0.1", "::1"]` — DNS-rebinding protection that rejects any inbound request whose `Host` header authority is not loopback (`tower.rs:114`). agentmem never overrides it.

In a Kubernetes cluster, clients reach the server through a Service DNS name (e.g. `agentmem.default.svc.cluster.local`), a pod IP, or an ingress hostname. None of these are loopback, so every request is rejected before reaching the MCP handler. There is currently no configuration knob to widen the allow-list, so the only workaround is to not use HTTP transport off-host at all.

`rmcp` already exposes the needed levers: `StreamableHttpServerConfig.allowed_hosts` (a `Vec<String>` of hostname or `host:port` authorities), `with_allowed_hosts(...)`, and `disable_allowed_hosts()` (which clears the list — an empty list means "allow all"). We only need to surface configuration and wire it through.

## Goals / Non-Goals

**Goals:**
- Let operators specify which `Host` values the HTTP transport accepts, via a new `AGENTMEM_HTTP_ALLOWED_HOSTS` env var and `--http-allowed-hosts` CLI flag.
- Preserve today's loopback-only default when the variable is unset (no behavior change for local development).
- Provide an explicit, auditable opt-out (`*`) for deployments that terminate `Host` trust upstream, with a startup warning.
- Keep the configuration shape consistent with existing HTTP variables (`AGENTMEM_HTTP_BIND`, `AGENTMEM_HTTP_BEARER`) and the comma-separated list convention already used by `AGENTMEM_INCLUDE_HIDDEN_GLOBS`.

**Non-Goals:**
- Origin (`Origin` header) validation — `rmcp`'s `allowed_origins` stays at its default (disabled). Out of scope; can follow later if browser clients need it.
- Authentication changes — bearer-token gating is unchanged.
- Per-route or wildcard-subdomain host matching beyond what `rmcp` already implements.

## Decisions

### Decision: New `AGENTMEM_HTTP_ALLOWED_HOSTS` variable, comma-separated
A single comma-separated variable mirrors the existing `AGENTMEM_INCLUDE_HIDDEN_GLOBS` convention and keeps the env surface flat. Each entry is trimmed; empty entries are dropped. Values are passed verbatim to `rmcp`, which already normalizes hosts and parses optional ports (`tower.rs:231-248`), so agentmem does not re-implement authority parsing.

*Alternative considered:* a JSON array or repeated flags. Rejected — heavier than the established comma-list idiom for what is usually one or two hostnames.

### Decision: Unset → keep rmcp's loopback default; do not pass a config
When the resolved list is empty, construct the service exactly as today (`StreamableHttpServerConfig::default()`), so the loopback default is preserved by `rmcp` itself rather than re-stated in agentmem. This guarantees the default tracks `rmcp` and that local development is byte-for-byte unchanged.

### Decision: `*` sentinel disables validation via `disable_allowed_hosts()`
Operators behind an ingress/proxy that already enforces host trust need a clean opt-out. The single value `*` maps to `StreamableHttpServerConfig::default().disable_allowed_hosts()` (empty list = allow all). Because this removes a security control, the server logs a single `WARN` line at startup, alongside the existing unauthenticated/non-loopback warnings. `*` is only meaningful as the sole entry; mixing `*` with explicit hosts resolves to "disabled" and is treated as the wildcard.

### Decision: Carry the list on `Transport::Http`
Add an `allowed_hosts: Vec<String>` field to the `Transport::Http` variant so the resolved configuration flows to `transport::http::serve` the same way `bind` and `bearer` already do. The `serve` signature gains the parameter; `transport/http.rs` builds the `StreamableHttpServerConfig` from it.

### Decision: Resolution and precedence reuse the existing pattern
Parsing lives in `src/config.rs` next to the other HTTP variables; the `--http-allowed-hosts` CLI flag overrides the env var, matching how `--http-bind` overrides `AGENTMEM_HTTP_BIND`.

## Risks / Trade-offs

- **Operators set `*` and expose an unauthenticated server off-host** → the existing non-loopback-without-bearer warning still fires, and a dedicated "Host validation disabled" warning is added; docs steer operators to list explicit hosts and/or set `AGENTMEM_HTTP_BEARER`.
- **Misconfigured host (typo, missing port) silently rejects all traffic** → mitigate with documentation and by logging the effective allowed-hosts list at startup so the mismatch is visible. `rmcp` matches port only when the allow-list entry includes one (`tower.rs:259-264`), so a bare hostname accepts any port; this is documented.
- **rmcp internals change** → we depend only on the public `with_allowed_hosts` / `disable_allowed_hosts` API and the documented default, not on internal matching behavior. Pinned at `rmcp` 1.7.

## Migration Plan

Additive and backward-compatible. Unset → identical behavior to today. To adopt in Kubernetes: set `AGENTMEM_HTTP_ALLOWED_HOSTS` to the Service/ingress hostname(s) the clients use (and bind `0.0.0.0` via `AGENTMEM_HTTP_BIND`). Rollback is removing the variable. No data or schema migration.

## Open Questions

- Should the startup log always echo the effective allowed-hosts list, or only at `DEBUG`? Leaning toward `INFO` for one line so operators can confirm the parse, consistent with the existing bind-address log.
