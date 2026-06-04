//! The MCP server: a [`rmcp::ServerHandler`] that advertises the nine tools and
//! dispatches `tools/call` to the [`crate::tools::Toolbox`].
//!
//! Domain errors (policy, not-found, edit preconditions, …) are surfaced as
//! structured *tool results* (`is_error = true` with a `code` field) so the agent
//! can read and react to them. Only an unknown tool name becomes a protocol-level
//! `method not found` error.

use std::sync::Arc;

use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParam,
    ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, model::JsonObject};

use crate::config::Config;
use crate::storage::Storage;
use crate::tools::Toolbox;

/// The MCP server handler. Cheap to clone — the shared [`Toolbox`] lives behind an
/// `Arc`, so the HTTP transport's per-session factory hands out lightweight
/// clones that all front the same storage layer and locks.
#[derive(Clone)]
pub struct AgentmemServer {
    toolbox: Arc<Toolbox>,
}

impl AgentmemServer {
    /// Build a server from a fully-resolved [`Config`].
    pub fn new(config: &Config) -> AgentmemServer {
        let storage = Storage::new(
            config.resolver(),
            config.honor_ignore_files,
            config.include_hidden,
        );
        let toolbox = Toolbox::new(storage, config.policy, config.timezone);
        AgentmemServer {
            toolbox: Arc::new(toolbox),
        }
    }
}

impl ServerHandler for AgentmemServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Durable, namespaced markdown memory for agents. Every tool call must \
                 carry the scope keys defined by the server's VFS template; paths are \
                 virtual and relative to the vault root."
                    .to_string(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.toolbox.list_tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let args: JsonObject = request.arguments.unwrap_or_default();
        match self.toolbox.call(&request.name, &args) {
            Some(Ok(result)) => Ok(result),
            Some(Err(err)) => Ok(err.into_tool_result()),
            None => Err(McpError::invalid_params(
                format!("unknown tool '{}'", request.name),
                None,
            )),
        }
    }
}
