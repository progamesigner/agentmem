//! The Streamable HTTP transport (default).
//!
//! Mounts the `rmcp` Streamable HTTP service at `POST`/`GET`/`DELETE /mcp` and a
//! plain `GET /v1/context` read endpoint behind an `axum` router that also serves
//! the Kubernetes-style `GET /healthz` (liveness) and `GET /readyz` (readiness)
//! probes. When `AGENTMEM_HTTP_BEARER` is set, an `axum` middleware enforces a
//! matching `Authorization: Bearer <token>` header on the `/mcp` and `/v1/context`
//! routes and returns HTTP 401 otherwise; the probe routes are always reachable.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Query, Request, State};
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};

use crate::error::AgentmemError;
use crate::mcp::AgentmemServer;

/// Serve over Streamable HTTP, binding `bind` until a termination signal arrives.
///
/// `allowed_hosts` configures the transport's inbound `Host` validation: an empty
/// list keeps `rmcp`'s loopback-only default (`localhost`, `127.0.0.1`, `::1`),
/// the sole entry `*` disables validation, and any other list is used verbatim.
pub async fn serve(
    bind: SocketAddr,
    bearer: Option<String>,
    allowed_hosts: Vec<String>,
    server: AgentmemServer,
) -> anyhow::Result<()> {
    if bearer.is_none() {
        tracing::warn!("AGENTMEM_HTTP_BEARER is unset; the HTTP endpoint is unauthenticated");
        if !bind.ip().is_loopback() {
            tracing::warn!(
                %bind,
                "binding a non-loopback interface without AGENTMEM_HTTP_BEARER; \
                 the endpoint is reachable off-host without authentication"
            );
        }
    }

    let http_config = if allowed_hosts.is_empty() {
        // No override: keep rmcp's loopback-only DNS-rebinding default.
        StreamableHttpServerConfig::default()
    } else if allowed_hosts == ["*"] {
        tracing::warn!(
            "AGENTMEM_HTTP_ALLOWED_HOSTS=* disables Host validation; \
             any Host header will be accepted"
        );
        StreamableHttpServerConfig::default().disable_allowed_hosts()
    } else {
        tracing::info!(allowed_hosts = %allowed_hosts.join(", "), "Host validation allow-list");
        StreamableHttpServerConfig::default().with_allowed_hosts(allowed_hosts)
    };

    let mcp_service = StreamableHttpService::new(
        {
            let server = server.clone();
            move || Ok(server.clone())
        },
        Arc::new(LocalSessionManager::default()),
        http_config,
    );

    // The gated sub-router carries the MCP service and the `/v1/context` read
    // endpoint; both inherit the bearer middleware when one is configured. The
    // `AgentmemServer` is wired in as handler state so `/v1/context` can reach
    // the shared renderer.
    // Ungated probes: liveness never depends on the index (so an orchestrator
    // won't kill a building pod), readiness reflects the eager index build.
    let probes = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .with_state(server.clone());

    let mut gated = Router::new()
        .route_service("/mcp", mcp_service)
        .route("/v1/context", get(context))
        .with_state(server);
    if let Some(token) = bearer {
        gated = gated.layer(from_fn_with_state(Arc::new(token), require_bearer));
    }

    let app = probes.merge(gated);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "serving MCP over Streamable HTTP");
    axum::serve(listener, app)
        .with_graceful_shutdown(super::shutdown_signal())
        .await?;
    Ok(())
}

/// Liveness route. Always succeeds once the process is up — it never depends on
/// the recall index, so a slow cold build cannot fail a liveness probe.
async fn healthz() -> &'static str {
    "ok"
}

/// Readiness route. Reports `200` only once the recall index is built (or
/// immediately when recall is disabled); `503` while the eager build is in flight,
/// so traffic is held until the server can serve recall.
async fn readyz(State(server): State<AgentmemServer>) -> Response {
    if server.recall_ready() {
        (StatusCode::OK, "ready").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "indexing").into_response()
    }
}

/// `GET /v1/context` — render the per-scope session-context bootstrap.
///
/// Each VFS-scheme placeholder is supplied as a query parameter (e.g.
/// `?agent=default&user=alice`); the scheme's placeholders are bound into the
/// scope in order. The same renderer that backs `load_session_context` produces
/// the body. Returns `text/markdown` by default, or `{ rendered, missing }` JSON
/// when the `Accept` header prefers `application/json`.
async fn context(
    State(server): State<AgentmemServer>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let placeholders = server.scheme_placeholders();

    // Reject any query parameter that is not a scheme placeholder.
    if let Some(unexpected) = params
        .keys()
        .find(|k| !placeholders.iter().any(|p| p == *k))
    {
        return error(
            StatusCode::BAD_REQUEST,
            format!("unexpected query parameter '{unexpected}'"),
        );
    }

    // Bind the placeholders, in scheme order. Absent or empty values fall through
    // to the renderer's own validation (MissingScope / InvalidArgument).
    let mut scope: BTreeMap<String, String> = BTreeMap::new();
    for ph in &placeholders {
        if let Some(value) = params.get(ph) {
            scope.insert(ph.clone(), value.clone());
        }
    }

    match server.render_session_context(&scope) {
        Ok(sc) => {
            if prefers_json(&headers) {
                Json(serde_json::json!({
                    "rendered": sc.rendered,
                    "missing": sc.missing,
                }))
                .into_response()
            } else {
                (
                    [(CONTENT_TYPE, "text/markdown; charset=utf-8")],
                    sc.rendered,
                )
                    .into_response()
            }
        }
        Err(err) => {
            let status = match err {
                AgentmemError::MissingScope { .. } | AgentmemError::InvalidArgument { .. } => {
                    StatusCode::BAD_REQUEST
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error(status, err.to_string())
        }
    }
}

/// `true` when the `Accept` header asks for `application/json`.
fn prefers_json(headers: &HeaderMap) -> bool {
    headers
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|a| a.contains("application/json"))
}

/// A `{ "error": <message> }` JSON response with the given status.
fn error(status: StatusCode, message: String) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// Reject requests whose bearer token does not match the configured secret.
async fn require_bearer(
    State(expected): State<Arc<String>>,
    request: Request,
    next: Next,
) -> Response {
    let presented = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    if presented == Some(expected.as_str()) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "jsonrpc": "2.0",
                "error": { "code": -32001, "message": "unauthorized: missing or invalid bearer token" },
                "id": null
            })),
        )
            .into_response()
    }
}
