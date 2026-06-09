## ADDED Requirements

### Requirement: HTTP transport Host validation
The system SHALL, when running under `http` transport, configure the `rmcp` Streamable HTTP service's inbound `Host` validation from the resolved allowed-hosts list (`AGENTMEM_HTTP_ALLOWED_HOSTS` / `--http-allowed-hosts`). When the list is non-empty, requests whose `Host` header authority matches an entry SHALL be accepted and all others SHALL be rejected by the transport. When the list is unset, the transport SHALL retain its loopback-only default (`localhost`, `127.0.0.1`, `::1`). The single value `*` SHALL disable `Host` validation so requests with any `Host` header are accepted.

This makes the HTTP transport usable behind a Kubernetes Service or ingress, where clients address the server by a cluster DNS name or external hostname rather than a loopback address.

#### Scenario: Cluster hostname accepted when allow-listed
- **WHEN** the server runs under `http` transport with `AGENTMEM_HTTP_ALLOWED_HOSTS=agentmem.svc.cluster.local` and a client sends `POST /mcp` carrying `Host: agentmem.svc.cluster.local`
- **THEN** the transport accepts the request and processes the MCP call

#### Scenario: Non-listed host rejected
- **WHEN** the server runs under `http` transport with `AGENTMEM_HTTP_ALLOWED_HOSTS=agentmem.example.com` and a client sends a request carrying `Host: evil.example.net`
- **THEN** the transport rejects the request

#### Scenario: Loopback default preserved when unset
- **WHEN** the server runs under `http` transport with `AGENTMEM_HTTP_ALLOWED_HOSTS` unset and a client on the same host sends `POST /mcp` carrying `Host: 127.0.0.1:8000`
- **THEN** the transport accepts the request, unchanged from prior behavior

#### Scenario: Validation disabled by wildcard
- **WHEN** the server runs under `http` transport with `AGENTMEM_HTTP_ALLOWED_HOSTS=*` and a client sends a request carrying any `Host` header
- **THEN** the transport accepts the request without `Host` validation
