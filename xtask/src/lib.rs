//! `FrixMCP` development commands (docs/PROJECT.md §6.1, §6.6).
//!
//! - [`redact`]: replaces MACs, serials, SSIDs, names, hostnames and the public IP in
//!   recorded responses, keeping the XML or JSON valid.
//! - [`record`]: runs the tool [`reads`] through a transport that records every
//!   exchange and lets through only Level Read actions.

pub mod reads;
pub mod record;
pub mod redact;
pub mod save;
