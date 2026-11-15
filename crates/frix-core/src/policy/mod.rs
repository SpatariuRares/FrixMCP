//! Policy: every router action has a Classification; in this phase only
//! `Exposed(Read)` actions are reachable (docs/PROJECT.md §4.2, §6.2; `GLOSSARY.md`).

mod registry;

use crate::error::FrixError;

/// Risk degree of an exposed operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Read: always enabled.
    Read,
    /// Low-risk write (later specs).
    Write,
    /// Dangerous write: requires Confirmation (later specs).
    Dangerous,
}

/// Decision taken for a router action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// The action is exposed with this Level.
    Exposed(Level),
    /// The action cannot be executed in any case.
    Forbidden,
}

/// A FRITZ!OS right of the Router Account, as in the rights column of
/// docs/PROJECT.md §4.2 (legend in §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Right {
    /// C: Configuration.
    Configuration,
    /// A: restricted configuration for applications.
    App,
    /// P: Phone contents and configuration.
    Phone,
    /// N: NAS contents and configuration.
    Nas,
    /// H: Home automation.
    HomeAuto,
}

impl Right {
    /// Name of the right as the Operator sees it in FRITZ!OS (§4.1, §4.3).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Configuration => "FRITZ!Box Settings",
            Self::App => "Restricted FRITZ!Box settings for apps",
            Self::Phone => "Telephony (Phone)",
            Self::Nas => "NAS",
            Self::HomeAuto => "Smart Home",
        }
    }
}

/// The rights that suffice, any one of them, to execute `action` of `service`
/// (§4.2, rights column; the instance does not matter). `Some(&[])`: no right (`-`).
/// `None`: §4.2 does not document this action's right.
#[must_use]
pub fn required_rights(service: &str, action: &str) -> Option<&'static [Right]> {
    let kind = service.split_once(':').map_or(service, |(kind, _)| kind);
    registry::RIGHTS
        .iter()
        .find(|(s, a, _)| *s == kind && *a == action)
        .map(|(_, _, r)| *r)
}

/// Classification of `action` of service `service` (`"DeviceInfo:1"` or `"DeviceInfo"`:
/// the instance does not matter, the three `WLANConfiguration` share the SCPD).
/// `None` if the action is not in the registry: the Policy denies it.
#[must_use]
pub fn classification(service: &str, action: &str) -> Option<Classification> {
    let kind = service.split_once(':').map_or(service, |(kind, _)| kind);
    registry::REGISTRY
        .iter()
        .find(|(s, a, _)| *s == kind && *a == action)
        .map(|(_, _, c)| *c)
}

/// The registry's (service without instance, action) pairs, in the order they appear.
#[cfg(test)]
pub(crate) fn registered_actions() -> impl Iterator<Item = (&'static str, &'static str)> {
    registry::REGISTRY.iter().map(|(s, a, _)| (*s, *a))
}

/// The (service, action) rows of the rights table, in order.
#[cfg(test)]
pub(crate) fn actions_with_rights() -> impl Iterator<Item = (&'static str, &'static str)> {
    registry::RIGHTS.iter().map(|(s, a, _)| (*s, *a))
}

/// Allows only actions classified `Exposed(Read)`.
pub(crate) fn authorize(service: &str, action: &str) -> Result<(), FrixError> {
    match classification(service, action) {
        Some(Classification::Exposed(Level::Read)) => Ok(()),
        Some(Classification::Exposed(level)) => Err(FrixError::Policy(format!(
            "{service}#{action} has Level {level:?}: only Read in this phase"
        ))),
        Some(Classification::Forbidden) => Err(FrixError::Policy(format!(
            "{service}#{action} is Forbidden"
        ))),
        None => Err(FrixError::Policy(format!(
            "{service}#{action} has no Classification"
        ))),
    }
}
