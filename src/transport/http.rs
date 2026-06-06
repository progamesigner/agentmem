//! The Streamable HTTP transport (default).
//!
//! Mounts the `rmcp` Streamable HTTP service at `POST`/`GET`/`DELETE /mcp` behind
//! an `axum` router that also serves a `GET /health` liveness route. When
//! `AGENTMEM_HTTP_BEARER` is set, an `axum` middleware enforces a matching
//! `Authorization: Bearer <token>` header on the `/mcp` route and returns HTTP 401
//! otherwise; `/health` is always reachable.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header::AUTHORIZATION};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};

use crate::mcp::AgentmemServer;

/// Serve over Streamable HTTP, binding `bind` until a termination signal arrives.
pub async fn serve(
    bind: SocketAddr,
    bearer: Option<String>,
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

    let mcp_service = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let mut mcp_router = Router::new().route_service("/mcp", mcp_service);
    if let Some(token) = bearer {
        mcp_router = mcp_router.layer(from_fn_with_state(Arc::new(token), require_bearer));
    }

    let app = Router::new()
        .route("/health", get(health))
        .merge(mcp_router);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "serving MCP over Streamable HTTP");
    axum::serve(listener, app)
        .with_graceful_shutdown(super::shutdown_signal())
        .await?;
    Ok(())
}

/// Liveness route.
async fn health() -> &'static str {
    "ok"
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
