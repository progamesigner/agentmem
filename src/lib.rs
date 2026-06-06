//! `agentmem` — an MCP server fronting an Obsidian-style markdown vault for
//! multi-tenant agent memory.
//!
//! See `openspec/changes/build-agentmem-mcp-server/` for the full specification.

pub mod config;
pub mod error;
pub mod mcp;
pub mod path;
pub mod policy;
pub mod scheme;
pub mod storage;
pub mod telemetry;
pub mod tools;
pub mod transport;

pub use error::{AgentmemError, ErrorCode};
pub use mcp::AgentmemServer;
