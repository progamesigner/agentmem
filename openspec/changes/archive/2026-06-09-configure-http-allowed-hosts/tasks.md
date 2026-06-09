## 1. Configuration surface

- [x] 1.1 Add `VAR_HTTP_ALLOWED_HOSTS = "AGENTMEM_HTTP_ALLOWED_HOSTS"` constant in `src/config.rs`
- [x] 1.2 Add `allowed_hosts: Vec<String>` field to the `Transport::Http` variant and update all constructions/matches (including `is_unauthenticated_non_loopback`, the debug/summary formatting around `src/config.rs:324`, and the default-config test helper around `src/config.rs:489`)
- [x] 1.3 Add `http_allowed_hosts: Option<String>` to the `Cli` struct (~`src/config.rs:96`) with a `--http-allowed-hosts` flag, and wire it into `as_overrides` alongside `http_bind`/`http_bearer` (~`src/config.rs:167`)
- [x] 1.4 Parse the env var in the `http` branch (~`src/config.rs:406-415`): split on `,`, trim entries, drop empties; an empty result leaves the list empty; a sole `*` is preserved as the disable sentinel; store on `Transport::Http.allowed_hosts`

## 2. Transport wiring

- [x] 2.1 Extend `transport::http::serve` (`src/transport/http.rs`) to accept the allowed-hosts list and update the caller in `src/transport/mod.rs:44`
- [x] 2.2 Build the `StreamableHttpServerConfig`: empty list → `::default()` (loopback default preserved); `["*"]` → `::default().disable_allowed_hosts()`; otherwise `::default().with_allowed_hosts(list)`
- [x] 2.3 Emit a `WARN` log line at startup when `Host` validation is disabled (`*`), and an `INFO` line echoing the effective allowed-hosts list when non-empty, consistent with the existing bind-address log

## 3. Tests

- [x] 3.1 In `src/config.rs` tests: allowed-hosts defaults to empty when unset; parsed-and-trimmed from a comma list; `*` retained as sentinel; CLI `--http-allowed-hosts` overrides the env var; stdio ignores the variable
- [x] 3.2 In `tests/http_transport.rs`: a request with a non-loopback `Host` is rejected by default; the same `Host` is accepted when listed in `AGENTMEM_HTTP_ALLOWED_HOSTS`; any `Host` is accepted under `*`; loopback `Host` still works when unset
- [x] 3.3 Run `cargo test` and `cargo clippy` and resolve failures

## 4. Documentation

- [x] 4.1 Document `AGENTMEM_HTTP_ALLOWED_HOSTS` / `--http-allowed-hosts` (semantics, comma list, `*` sentinel, bare-host-matches-any-port note) in README/config docs with a Kubernetes Service/ingress example
- [x] 4.2 Update any deployment notes (container image / k8s guidance) to mention setting allowed hosts alongside a non-loopback `AGENTMEM_HTTP_BIND`
