//! Language of the messages for the Operator (`language` in `config.toml`).
//!
//! Only what the Operator reads on stderr is localized: startup errors, help and the
//! startup summary. What the Agent reads (tool descriptions, `message` in tool errors)
//! stays in English, so that responses do not depend on the Operator's language.

use serde::Deserialize;

/// Language of the Operator's messages; English unless `config.toml` says otherwise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// English (default).
    #[default]
    En,
    /// Italian.
    It,
}
