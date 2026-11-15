//! One module per TR-064 service, with typed functions over [`Tr064Client`](crate::tr064::Tr064Client).
//!
//! Raw service and action strings do not leave these modules.
//! A new action is added here and in the Policy registry (`policy::registry`).

use crate::error::FrixError;

mod device_config;
pub mod device_info;
pub mod homeauto;
pub mod host_filter;
pub mod hosts;
pub mod lan_config_security;
pub mod lan_host_config;
mod list_path;
pub mod time;
pub mod user_interface;
pub mod wan_common;
pub mod wan_connection;
pub mod wan_dsl;
pub mod wlan;

/// A list read from the router entry by entry (one `Generic…` action per index), with an
/// explicit entry cap: a misbehaving router cannot cause thousands of calls.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryList<T> {
    /// The entries read and parsed, in index order.
    pub entries: Vec<T>,
    /// The entries skipped as unparsable: the others stay valid.
    pub skipped: Vec<SkippedEntry>,
    /// `Some` if the cap stopped the read before the end of the list.
    pub truncation: Option<EntryTruncation>,
}

impl<T> Default for EntryList<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            skipped: Vec::new(),
            truncation: None,
        }
    }
}

impl<T> EntryList<T> {
    /// Adds entry `index`, or marks it as skipped if it is unparsable
    /// ([`FrixError::Parse`]). Any other error (transport, rights, …) stops the read.
    pub(crate) fn push(
        &mut self,
        index: u16,
        entry: Result<T, FrixError>,
    ) -> Result<(), FrixError> {
        match entry {
            Ok(entry) => self.entries.push(entry),
            Err(FrixError::Parse(reason)) => self.skipped.push(SkippedEntry { index, reason }),
            Err(error) => return Err(error),
        }
        Ok(())
    }
}

/// A skipped entry: its index in the router and why it is unparsable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedEntry {
    /// Index of the entry (the `New…Index` argument).
    pub index: u16,
    /// The reason, text produced by `frix-core`.
    pub reason: String,
}

/// The entry cap stopped the read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryTruncation {
    /// The cap: maximum entries read.
    pub limit: usize,
    /// Entries not read, if the router declares the total; `None` if unknown.
    pub omitted: Option<usize>,
}
