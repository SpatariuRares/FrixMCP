//! `frix-mcp`: server MCP su stdio per il FRITZ!Box (docs/PROJECT.md §6.3).
//!
//! La libreria contiene il server; il binario si occupa solo dell'avvio
//! ([`cli`] e [`startup`]: config, Keychain, discovery). stdout è riservato al protocollo MCP.

pub mod cli;
mod response;
mod sanitize;
mod server;
pub mod startup;
mod tools;

pub use server::FrixServer;
