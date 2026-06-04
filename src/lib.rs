//! `agentmem` — an MCP server fronting an Obsidian-style markdown vault for
//! multi-tenant agent memory.
//!
//! See `openspec/changes/build-agentmem-mcp-server/` for the full specification.

pub mod config;
pub mod error;
pub mod path;
pub mod policy;
pub mod template;

pub use error::{AgentmemError, ErrorCode};
