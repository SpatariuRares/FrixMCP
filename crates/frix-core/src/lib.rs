//! `frix-core`: client TR-064 verso il FRITZ!Box, Policy e tipi di dominio.
//!
//! Non dipende da `frix-mcp` né da `frix-web` (docs/PROJECT.md §6.1).
//!
//! Percorso di una lettura: `services::*` → [`tr064::Tr064Client`] (SCPD, Policy,
//! envelope SOAP) → [`transport::HttpTransport`], l'unico seam verso il router.
#![warn(missing_docs)]

pub mod config;
pub mod error;
pub mod model;
pub mod policy;
pub mod secrets;
pub mod services;
pub mod tr064;
pub mod transport;

pub use error::FrixError;
