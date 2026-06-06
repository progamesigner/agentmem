//! The MCP server: a [`rmcp::ServerHandler`] that advertises the tools, the
//! `session-context` resource, and the `session-context` prompt, and dispatches
//! requests to the shared [`crate::tools::Toolbox`].
//!
//! Domain errors from `tools/call` (policy, not-found, edit preconditions, …) are
//! surfaced as structured *tool results* (`is_error = true` with a `code` field)
//! so the agent can read and react to them. Only an unknown tool name becomes a
//! protocol-level `method not found` error. The resource and prompt surfaces,
//! which have no structured-result channel, map domain errors to protocol errors.

use std::collections::BTreeMap;
use std::sync::Arc;

use rmcp::ServerHandler;
use rmcp::model::{
    AnnotateAble, CallToolRequestParams, CallToolResult, GetPromptRequestParams, GetPromptResult,
    ListPromptsResult, ListResourceTemplatesResult, ListToolsResult, PaginatedRequestParams,
    Prompt, PromptArgument, PromptMessage, PromptMessageRole, RawResourceTemplate,
    ReadResourceRequestParams, ReadResourceResult, ResourceContents, ServerCapabilities,
    ServerInfo,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, model::JsonObject};

use crate::config::Config;
use crate::error::AgentmemError;
use crate::storage::Storage;
use crate::tools::Toolbox;

/// The URI prefix for the session-context resource (note the trailing slash; the
/// per-scope segments follow it).
const SESSION_CONTEXT_URI_PREFIX: &str = "agentmem://session-context/";
/// The shared name of the session-context resource and prompt.
const SESSION_CONTEXT_NAME: &str = "session-context";

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
        let toolbox = Toolbox::new(
            storage,
            config.policy,
            config.timezone,
            config.session_context_template_file.clone(),
        );
        AgentmemServer {
            toolbox: Arc::new(toolbox),
        }
    }

    /// The `agentmem://session-context/{k1}/{k2}/…` URI template for the active
    /// scheme; the params follow the scheme's placeholders in order.
    fn session_context_uri_template(&self) -> String {
        let params: Vec<String> = self
            .toolbox
            .scheme_placeholders()
            .iter()
            .map(|k| format!("{{{k}}}"))
            .collect();
        format!("{SESSION_CONTEXT_URI_PREFIX}{}", params.join("/"))
    }

    /// Map the scheme's placeholders onto the path segments of a concrete
    /// session-context URI, returning the scope map. Errors if the URI does not
    /// carry exactly one segment per placeholder.
    fn scope_from_uri(&self, uri: &str) -> Result<BTreeMap<String, String>, McpError> {
        let rest = uri
            .strip_prefix(SESSION_CONTEXT_URI_PREFIX)
            .ok_or_else(|| {
                McpError::invalid_params(format!("unknown resource URI '{uri}'"), None)
            })?;
        let placeholders = self.toolbox.scheme_placeholders();
        let segments: Vec<&str> = if rest.is_empty() {
            Vec::new()
        } else {
            rest.split('/').collect()
        };
        if segments.len() != placeholders.len() {
            return Err(McpError::invalid_params(
                format!(
                    "resource URI '{uri}' has {} scope segment(s), expected {}",
                    segments.len(),
                    placeholders.len()
                ),
                None,
            ));
        }
        Ok(placeholders
            .into_iter()
            .zip(segments)
            .map(|(k, v)| (k, v.to_string()))
            .collect())
    }

    /// Read the scheme's placeholders out of a prompt's `arguments` object.
    fn scope_from_prompt_args(
        &self,
        arguments: &Option<JsonObject>,
    ) -> Result<BTreeMap<String, String>, McpError> {
        let mut scope = BTreeMap::new();
        let args = arguments.as_ref();
        for key in self.toolbox.scheme_placeholders() {
            match args.and_then(|a| a.get(&key)) {
                Some(serde_json::Value::String(s)) => {
                    scope.insert(key, s.clone());
                }
                Some(_) => {
                    return Err(McpError::invalid_params(
                        format!("prompt argument '{key}' must be a string"),
                        None,
                    ));
                }
                None => {
                    return Err(McpError::invalid_params(
                        format!("missing required prompt argument '{key}'"),
                        None,
                    ));
                }
            }
        }
        Ok(scope)
    }
}

/// Map a domain error onto a protocol error for the resource/prompt surfaces.
fn to_mcp_error(err: AgentmemError) -> McpError {
    McpError::invalid_params(err.to_string(), None)
}

impl ServerHandler for AgentmemServer {
    fn get_info(&self) -> ServerInfo {
        // `ServerInfo` is `#[non_exhaustive]`, so it cannot be built with a struct
        // expression here; start from its `Default` (which already sets the protocol
        // version and `Implementation::from_build_env()`) and override the rest.
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .enable_prompts()
            .build();
        info.instructions = Some(
            "Durable, namespaced markdown memory for agents. Every tool call must \
             carry the scope keys defined by the server's VFS scheme; paths are \
             virtual and relative to the vault root. The `session-context` resource \
             and prompt render the per-scope bootstrap."
                .to_string(),
        );
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.toolbox.list_tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
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

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let template = RawResourceTemplate {
            uri_template: self.session_context_uri_template(),
            name: SESSION_CONTEXT_NAME.to_string(),
            title: Some("Session context".to_string()),
            description: Some(
                "The rendered session-context bootstrap for a scope: foundational \
                 files woven into the configured template with a memory-tools guide."
                    .to_string(),
            ),
            mime_type: Some("text/markdown".to_string()),
            icons: None,
        }
        .no_annotation();
        Ok(ListResourceTemplatesResult::with_all_items(vec![template]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let scope = self.scope_from_uri(&request.uri)?;
        let sc = self
            .toolbox
            .render_session_context(&scope)
            .map_err(to_mcp_error)?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            sc.rendered,
            request.uri,
        )]))
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let arguments: Vec<PromptArgument> = self
            .toolbox
            .scheme_placeholders()
            .into_iter()
            .map(|key| {
                let description = format!("Scope key '{key}' identifying the caller.");
                PromptArgument::new(key)
                    .with_description(description)
                    .with_required(true)
            })
            .collect();
        let prompt = Prompt::new(
            SESSION_CONTEXT_NAME,
            Some("Render the per-scope session-context bootstrap."),
            Some(arguments),
        );
        Ok(ListPromptsResult::with_all_items(vec![prompt]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        if request.name != SESSION_CONTEXT_NAME {
            return Err(McpError::invalid_params(
                format!("unknown prompt '{}'", request.name),
                None,
            ));
        }
        let scope = self.scope_from_prompt_args(&request.arguments)?;
        let sc = self
            .toolbox
            .render_session_context(&scope)
            .map_err(to_mcp_error)?;
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            sc.rendered,
        )])
        .with_description("Session-context bootstrap."))
    }
}
