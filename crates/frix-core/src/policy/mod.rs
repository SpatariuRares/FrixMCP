//! Policy: ogni azione del router ha una Classification; in questa fase solo
//! le azioni `Exposed(Read)` sono raggiungibili (docs/PROJECT.md §4.2, §6.2; `GLOSSARY.md`).

mod registry;

use crate::error::FrixError;

/// Grado di rischio di un'operazione esposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Lettura: sempre attiva.
    Read,
    /// Scrittura a basso rischio (spec successive).
    Write,
    /// Scrittura pericolosa: richiede Confirmation (spec successive).
    Dangerous,
}

/// Decisione presa per un'azione del router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// L'azione è esposta con questo Level.
    Exposed(Level),
    /// L'azione non è eseguibile in nessun caso.
    Forbidden,
}

/// Un diritto FRITZ!OS del Router Account, come nella colonna "Diritto" della
/// docs/PROJECT.md §4.2 (legenda in §4.1).
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
    /// Nome del diritto come lo vede l'Operator in FRITZ!OS (§4.1, §4.3).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Configuration => "Impostazioni FRITZ!Box",
            Self::App => "Impostazioni FRITZ!Box limitate per app",
            Self::Phone => "Telefonia (Phone)",
            Self::Nas => "NAS",
            Self::HomeAuto => "Smart Home",
        }
    }
}

/// I diritti che bastano, uno qualunque, per eseguire `action` di `service`
/// (§4.2, colonna "Diritto"; l'istanza non conta). `Some(&[])`: nessun diritto (`-`).
/// `None`: la §4.2 non documenta il diritto di questa azione.
#[must_use]
pub fn required_rights(service: &str, action: &str) -> Option<&'static [Right]> {
    let tipo = service.split_once(':').map_or(service, |(tipo, _)| tipo);
    registry::RIGHTS
        .iter()
        .find(|(s, a, _)| *s == tipo && *a == action)
        .map(|(_, _, r)| *r)
}

/// Classification di `action` del servizio `service` (`"DeviceInfo:1"` o `"DeviceInfo"`:
/// l'istanza non conta, le tre `WLANConfiguration` condividono lo SCPD).
/// `None` se l'azione non è nel registro: la Policy la nega.
#[must_use]
pub fn classification(service: &str, action: &str) -> Option<Classification> {
    let tipo = service.split_once(':').map_or(service, |(tipo, _)| tipo);
    registry::REGISTRY
        .iter()
        .find(|(s, a, _)| *s == tipo && *a == action)
        .map(|(_, _, c)| *c)
}

/// Le coppie (servizio senza istanza, azione) del registro, nell'ordine in cui compaiono.
#[cfg(test)]
pub(crate) fn registered_actions() -> impl Iterator<Item = (&'static str, &'static str)> {
    registry::REGISTRY.iter().map(|(s, a, _)| (*s, *a))
}

/// Le righe (servizio, azione) della tabella dei diritti, in ordine.
#[cfg(test)]
pub(crate) fn actions_with_rights() -> impl Iterator<Item = (&'static str, &'static str)> {
    registry::RIGHTS.iter().map(|(s, a, _)| (*s, *a))
}

/// Consente solo le azioni classificate `Exposed(Read)`.
pub(crate) fn authorize(service: &str, action: &str) -> Result<(), FrixError> {
    match classification(service, action) {
        Some(Classification::Exposed(Level::Read)) => Ok(()),
        Some(Classification::Exposed(level)) => Err(FrixError::Policy(format!(
            "{service}#{action} ha Level {level:?}: in questa fase solo Read"
        ))),
        Some(Classification::Forbidden) => {
            Err(FrixError::Policy(format!("{service}#{action} è Forbidden")))
        }
        None => Err(FrixError::Policy(format!(
            "{service}#{action} non ha Classification"
        ))),
    }
}
