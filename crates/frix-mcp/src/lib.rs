//! `frix-mcp`: MCP server on stdio for the FRITZ!Box (docs/PROJECT.md §6.3).
//!
//! The library holds the server; the binary only handles startup
//! ([`cli`] and [`startup`]: config, keyring, discovery). stdout is reserved for the MCP protocol.

pub mod cli;
mod response;
mod sanitize;
mod server;
pub mod startup;
mod tools;

pub use server::FrixServer;
