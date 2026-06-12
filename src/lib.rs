//! `agentmem` — an MCP server fronting a plain-markdown vault for
//! multi-tenant agent memory.
//!
//! See `openspec/changes/build-agentmem-mcp-server/` for the full specification.

pub mod config;
pub mod error;
pub mod frontmatter;
pub mod mcp;
pub mod path;
pub mod policy;
pub mod recall;
pub mod scheme;
pub mod session_context;
pub mod storage;
pub mod telemetry;
pub mod template;
pub mod tools;
pub mod transport;
pub mod wikilink;

pub use error::{AgentmemError, ErrorCode};
pub use mcp::AgentmemServer;
