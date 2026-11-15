//! `frix-core`: TR-064 client for the FRITZ!Box, Policy and domain types.
//!
//! Does not depend on `frix-mcp` or `frix-web` (docs/PROJECT.md §6.1).
//!
//! Path of a read: `services::*` → [`tr064::Tr064Client`] (SCPD, Policy,
//! SOAP envelope) → [`transport::HttpTransport`], the only seam to the router.
#![warn(missing_docs)]

pub mod config;
pub mod error;
pub mod language;
pub mod model;
pub mod policy;
pub mod secrets;
pub mod services;
pub mod tr064;
pub mod transport;

pub use error::FrixError;
