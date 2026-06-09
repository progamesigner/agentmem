## Why

The HTTP transport constructs the `rmcp` Streamable HTTP service with `StreamableHttpServerConfig::default()`, whose `allowed_hosts` defaults to loopback only (`localhost`, `127.0.0.1`, `::1`). This is rmcp's DNS-rebinding protection. When agentmem runs in a Kubernetes cluster, clients reach it through a Service DNS name or external hostname, so the inbound `Host` header never matches the loopback allow-list and rmcp rejects every request. There is currently no way to tell agentmem which hosts are legitimate, so the HTTP transport is effectively unusable off-host.

## What Changes

- Add a new `AGENTMEM_HTTP_ALLOWED_HOSTS` environment variable (with a matching `--http-allowed-hosts` CLI flag) accepting a comma-separated list of allowed `Host` authorities (hostname or `host:port`), used only under the `http` transport.
- When set, pass the parsed list to `StreamableHttpServerConfig.allowed_hosts` so requests carrying those `Host` values are accepted.
- When unset, preserve today's behavior: rmcp's loopback-only default (`localhost`, `127.0.0.1`, `::1`) continues to apply, so local development keeps working unchanged.
- Support an explicit opt-out value (e.g. `*`) that disables `Host` validation for deployments that terminate trust at an upstream proxy/ingress; emit a startup `WARN` when validation is disabled.
- Document the variable and the Kubernetes deployment guidance in the configuration surface.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `configuration`: add `AGENTMEM_HTTP_ALLOWED_HOSTS` / `--http-allowed-hosts` to the recognized configuration surface, including parsing, the disable sentinel, and the precedence rule shared by other HTTP options.
- `mcp-server`: the HTTP transport SHALL apply the configured allowed-hosts list to the `rmcp` Streamable HTTP service's `Host` validation, accepting matching hosts and rejecting others; default behavior (loopback-only) is unchanged.

## Impact

- Code: `src/config.rs` (new variable, CLI flag, parsing, `Transport::Http` carries the allowed-hosts list), `src/transport/http.rs` (build `StreamableHttpServerConfig` from the list instead of `::default()`).
- Dependency: relies on existing `rmcp` 1.7 `StreamableHttpServerConfig::with_allowed_hosts` / `disable_allowed_hosts` APIs — no new dependency.
- Docs/specs: `openspec/specs/configuration` and `openspec/specs/mcp-server`; README/deployment notes for Kubernetes.
- No breaking change: unset behavior is identical to today.
